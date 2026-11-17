use std::{collections::HashMap, fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}};

use crate::rules::{context::{ActionCallback, ConditionCallback, Context, CtxPostAction, CtxPreAction, PostactionCallback, PostconditionCallback, PreactionCallback, PreconditionCallback}, errors::{CallbackError, CallbackResult, ModError, RuleManagerError, RuleManagerResult}, rules::{ActionType, MatchType, PreparseCondition, PreparseModification, PreparseRule, PreparseRules, Rule, RuleType}};

pub struct RuleManager {
    rule_filepath: PathBuf,
    rules: Vec<Rule>,

    preconditions: HashMap<String, PreconditionCallback>,
    postconditions: HashMap<String, PostconditionCallback>,

    preactions: HashMap<String, PreactionCallback>,
    postactions: HashMap<String, PostactionCallback>,
}

impl RuleManager {
    pub fn new<P: AsRef<Path>>(rule_filepath: P) -> RuleManagerResult<Self> {
        if !rule_filepath.as_ref().is_file() {
            if let Some(dir) = rule_filepath.as_ref().parent()
            && !dir.is_dir()
            {
                fs::create_dir_all(dir)
                    .map_err(RuleManagerError::WriteError)?;
            }

            fs::File::create_new(&rule_filepath)
                .map_err(RuleManagerError::Io)
                .and_then(|mut f| {
                    let yaml_string = yaml_serde::to_string(&Self::default_rules())
                        .map_err(RuleManagerError::YamlParseError)?;

                    f.write(yaml_string.as_bytes())
                        .map_err(RuleManagerError::WriteError)
                })?;

            println!("Created new rules file at {:?}", rule_filepath.as_ref());
        } else {
            println!("Using rules file at {:?}", rule_filepath.as_ref());
        }

        let rules = Self::read_rules(&rule_filepath)?;


        Ok(Self {
            rule_filepath: rule_filepath.as_ref().to_path_buf(),
            rules,

            preconditions: HashMap::new(),
            postconditions: HashMap::new(),

            preactions: HashMap::new(),
            postactions: HashMap::new()
        })
    }


    pub fn reload_rules(&mut self) -> RuleManagerResult<()> {
        self.rules = Self::read_rules(&self.rule_filepath)?;
        Ok(())
    }

    fn read_rules<P: AsRef<Path>>(rule_file: P) -> RuleManagerResult<Vec<Rule>> {
        let f = OpenOptions::new()
            .read(true)
            .open(rule_file)
            .map_err(RuleManagerError::Io)?;

        let preparse: PreparseRules = yaml_serde::from_reader(f)
            .map_err(RuleManagerError::YamlParseError)?;

        Ok(preparse.parse())
    }


    fn default_rules() -> PreparseRules {
        let wait_50_pct = PreparseRule {
            inactive: false,
            rule_type: RuleType::PostAction,
            condition_match_type: MatchType::MatchAll,
            conditions: vec![
                PreparseCondition::ActionType(ActionType::Readdir)
            ],
            modifications: vec![
                PreparseModification::WaitPct(50.0)
            ]
        };

        let mut rule_map = HashMap::new();
        rule_map.insert("Make Readdir take 50% longer".to_string(), wait_50_pct);

        PreparseRules(rule_map)
    }


    pub fn add_precondition(
        &mut self,
        cb_name: &str,
        cb: PreconditionCallback
    ) {
        self.preconditions.insert(cb_name.to_string(), cb);
    }

    pub fn add_postcondition(
        &mut self,
        cb_name: &str,
        cb: PostconditionCallback
    ) {
        self.postconditions.insert(cb_name.to_string(), cb);
    }

    pub fn add_preaction(
        &mut self,
        cb_name: &str,
        cb: PreactionCallback
    ) {
        self.preactions.insert(cb_name.to_string(), cb);
    }

    pub fn add_postaction(
        &mut self,
        cb_name: &str,
        cb: PostactionCallback
    ) {
        self.postactions.insert(cb_name.to_string(), cb);
    }


    fn find_matching_rules<CT>(
        &self,
        cond_callbacks: &HashMap<String, ConditionCallback<CT>>,
        ctx: &Context<CT>,
        rule_type: RuleType
    ) -> impl Iterator<Item = &Rule>
    {
        self.rules
            .iter()
            .filter(move |rule| {
                rule.rule_type == rule_type
                && rule.conditions_match(cond_callbacks, ctx)
            })
    }

    fn call_callbacks<CT>(
        &self,
        action_callbacks: &HashMap<String, ActionCallback<CT>>,
        cond_callbacks: &HashMap<String, ConditionCallback<CT>>,
        ctx: &mut Context<CT>,
        rule_type: RuleType
    ) -> Vec<CallbackError> {
        // Find all rules that match given conditions
        // Convert these into a vec of pairs of rule_name, modification (String, &Modification)
        let mods: Vec<_> = self.find_matching_rules(
            cond_callbacks,
            ctx,
            rule_type
        )
            .flat_map(|rule| {
                rule.modifications
                    .iter()
                    .map(|m| (&rule.name, m))
            })
            .collect();

        let mut errors = Vec::new();
        for (rule_name, m) in mods {
            let fn_name = &m.0.fn_name;
            let res = match action_callbacks.get(fn_name) {
                Some(cb) => cb(m.0.args.clone(), ctx),
                None => Err(ModError::CallbackNotFound)
            };

            if let Err(e) = res {
                errors.push(e.into_cb_error(rule_name.clone(), fn_name.clone()));
            }
        }

        errors
    }

    pub fn call_preactions(
        &self,
        ctx: &mut Context<CtxPreAction>
    ) -> Vec<CallbackError> {
        self.call_callbacks(
            &self.preactions,
            &self.preconditions,
            ctx,
            RuleType::PreAction
        )
    }

    pub fn call_postactions(
        &self,
        ctx: &mut Context<CtxPostAction>
    ) -> Vec<CallbackError> {
        self.call_callbacks(
            &self.postactions,
            &self.postconditions,
            ctx,
            RuleType::PostAction
        )
    }
}
