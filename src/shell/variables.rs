//! Typed shell variable storage.
//!
//! GNU Bash references: `variables.c`, `array.c`, `assoc.c`, and
//! `builtins/declare.def`. The executor's legacy environment map remains an
//! adapter while callers migrate to this owner.

use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellValue {
    Scalar(String),
    IndexedArray(BTreeMap<i64, String>),
    AssociativeArray(HashMap<String, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub value: ShellValue,
    pub exported: bool,
    pub readonly: bool,
    pub integer: bool,
    pub nameref: Option<String>,
    pub uppercase: bool,
    pub lowercase: bool,
}

impl Variable {
    pub fn scalar(value: impl Into<String>) -> Self {
        Self {
            value: ShellValue::Scalar(value.into()),
            exported: false,
            readonly: false,
            integer: false,
            nameref: None,
            uppercase: false,
            lowercase: false,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VariableStore {
    variables: BTreeMap<String, Variable>,
}

impl VariableStore {
    pub fn get(&self, name: &str) -> Option<&Variable> {
        self.variables.get(name)
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Variable> {
        self.variables.get_mut(name)
    }

    pub fn set(&mut self, name: impl Into<String>, variable: Variable) -> Result<(), &'static str> {
        let name = name.into();
        if self.get(&name).map_or(false, |old| old.readonly) {
            return Err("readonly variable");
        }
        self.variables.insert(name, variable);
        Ok(())
    }

    pub fn set_scalar(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), &'static str> {
        self.set(name, Variable::scalar(value))
    }

    pub fn unset(&mut self, name: &str) -> Result<Option<Variable>, &'static str> {
        if self.get(name).map_or(false, |variable| variable.readonly) {
            return Err("readonly variable");
        }
        Ok(self.variables.remove(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Variable)> {
        self.variables.iter()
    }

    /// Serialize only exported scalar values for a child process.
    pub fn export_environment(&self) -> HashMap<String, String> {
        self.variables
            .iter()
            .filter_map(|(name, variable)| {
                if !variable.exported {
                    return None;
                }
                let ShellValue::Scalar(value) = &variable.value else {
                    return None;
                };
                let value = if variable.uppercase {
                    value.to_uppercase()
                } else if variable.lowercase {
                    value.to_lowercase()
                } else {
                    value.clone()
                };
                Some((name.clone(), value))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_and_associative_values_are_typed() {
        let mut store = VariableStore::default();
        let mut indexed = BTreeMap::new();
        indexed.insert(4, "four".into());
        store
            .set(
                "indexed",
                Variable {
                    value: ShellValue::IndexedArray(indexed),
                    ..Variable::scalar("")
                },
            )
            .unwrap();
        let mut associative = HashMap::new();
        associative.insert("key".into(), "value".into());
        store
            .set(
                "assoc",
                Variable {
                    value: ShellValue::AssociativeArray(associative),
                    ..Variable::scalar("")
                },
            )
            .unwrap();
        assert!(matches!(
            store.get("indexed").unwrap().value,
            ShellValue::IndexedArray(_)
        ));
        assert!(matches!(
            store.get("assoc").unwrap().value,
            ShellValue::AssociativeArray(_)
        ));
    }

    #[test]
    fn export_is_an_adapter_not_array_storage() {
        let mut store = VariableStore::default();
        let mut variable = Variable::scalar("mixed");
        variable.exported = true;
        variable.uppercase = true;
        store.set("value", variable).unwrap();
        assert_eq!(store.export_environment()["value"], "MIXED");
    }
}
