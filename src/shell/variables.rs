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

    pub fn indexed_element(&self, name: &str, index: i64) -> Option<&str> {
        match &self.get(name)?.value {
            ShellValue::IndexedArray(values) => values.get(&index).map(String::as_str),
            _ => None,
        }
    }

    pub fn associative_element(&self, name: &str, key: &str) -> Option<&str> {
        match &self.get(name)?.value {
            ShellValue::AssociativeArray(values) => values.get(key).map(String::as_str),
            _ => None,
        }
    }

    pub fn set_indexed_element(
        &mut self,
        name: &str,
        index: i64,
        value: impl Into<String>,
    ) -> Result<(), &'static str> {
        let variable = self.variables.entry(name.to_string()).or_insert_with(|| Variable {
            value: ShellValue::IndexedArray(BTreeMap::new()),
            ..Variable::scalar("")
        });
        if variable.readonly {
            return Err("readonly variable");
        }
        if matches!(variable.value, ShellValue::Scalar(_)) {
            variable.value = ShellValue::IndexedArray(BTreeMap::new());
        }
        match &mut variable.value {
            ShellValue::IndexedArray(values) => {
                values.insert(index, value.into());
                Ok(())
            }
            ShellValue::AssociativeArray(_) => Err("cannot assign indexed element to associative array"),
            ShellValue::Scalar(_) => unreachable!(),
        }
    }

    pub fn replace_indexed_array<I, V>(
        &mut self,
        name: &str,
        values: I,
    ) -> Result<(), &'static str>
    where
        I: IntoIterator<Item = V>,
        V: Into<String>,
    {
        let variable = self.variables.entry(name.to_string()).or_insert_with(|| Variable {
            value: ShellValue::IndexedArray(BTreeMap::new()),
            ..Variable::scalar("")
        });
        if variable.readonly {
            return Err("readonly variable");
        }
        if !matches!(variable.value, ShellValue::IndexedArray(_)) {
            variable.value = ShellValue::IndexedArray(BTreeMap::new());
        }
        let ShellValue::IndexedArray(entries) = &mut variable.value else {
            unreachable!();
        };
        entries.clear();
        for (index, value) in values.into_iter().enumerate() {
            entries.insert(index as i64, value.into());
        }
        Ok(())
    }

    pub fn replace_associative_array<I, K, V>(
        &mut self,
        name: &str,
        values: I,
    ) -> Result<(), &'static str>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let variable = self.variables.entry(name.to_string()).or_insert_with(|| Variable {
            value: ShellValue::AssociativeArray(HashMap::new()),
            ..Variable::scalar("")
        });
        if variable.readonly {
            return Err("readonly variable");
        }
        if !matches!(variable.value, ShellValue::AssociativeArray(_)) {
            variable.value = ShellValue::AssociativeArray(HashMap::new());
        }
        let ShellValue::AssociativeArray(entries) = &mut variable.value else {
            unreachable!();
        };
        entries.clear();
        for (key, value) in values {
            entries.insert(key.into(), value.into());
        }
        Ok(())
    }

    pub fn set_associative_element(
        &mut self,
        name: &str,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), &'static str> {
        let variable = self.variables.entry(name.to_string()).or_insert_with(|| Variable {
            value: ShellValue::AssociativeArray(HashMap::new()),
            ..Variable::scalar("")
        });
        if variable.readonly {
            return Err("readonly variable");
        }
        if matches!(variable.value, ShellValue::Scalar(_)) {
            variable.value = ShellValue::AssociativeArray(HashMap::new());
        }
        match &mut variable.value {
            ShellValue::AssociativeArray(values) => {
                values.insert(key.into(), value.into());
                Ok(())
            }
            ShellValue::IndexedArray(_) => Err("cannot assign associative element to indexed array"),
            ShellValue::Scalar(_) => unreachable!(),
        }
    }

    pub fn remove_indexed_element(
        &mut self,
        name: &str,
        index: i64,
    ) -> Result<Option<String>, &'static str> {
        let Some(variable) = self.get_mut(name) else {
            return Ok(None);
        };
        if variable.readonly {
            return Err("readonly variable");
        }
        match &mut variable.value {
            ShellValue::IndexedArray(values) => Ok(values.remove(&index)),
            _ => Ok(None),
        }
    }

    pub fn remove_associative_element(
        &mut self,
        name: &str,
        key: &str,
    ) -> Result<Option<String>, &'static str> {
        let Some(variable) = self.get_mut(name) else {
            return Ok(None);
        };
        if variable.readonly {
            return Err("readonly variable");
        }
        match &mut variable.value {
            ShellValue::AssociativeArray(values) => Ok(values.remove(key)),
            _ => Ok(None),
        }
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

    /// Import the process environment as exported scalar shell variables.
    ///
    /// This is the initial migration boundary: executor writes still update the
    /// legacy environment mirror, while typed array and attribute state can be
    /// introduced without changing child-process setup in one step.
    pub fn from_environment(environment: &HashMap<String, String>) -> Self {
        let mut store = Self::default();
        for (name, value) in environment {
            let mut variable = Variable::scalar(value.clone());
            variable.exported = true;
            store.variables.insert(name.clone(), variable);
        }
        store
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

    #[test]
    fn element_apis_preserve_sparse_and_readonly_semantics() {
        let mut store = VariableStore::default();
        store.set_indexed_element("items", 3, "three").unwrap();
        store.set_indexed_element("items", 9, "nine").unwrap();
        assert_eq!(store.indexed_element("items", 3), Some("three"));
        assert_eq!(store.indexed_element("items", 4), None);
        assert_eq!(store.remove_indexed_element("items", 3).unwrap(), Some("three".into()));
        assert_eq!(store.indexed_element("items", 3), None);

        store.set_associative_element("map", "a b]", "value").unwrap();
        assert_eq!(store.associative_element("map", "a b]"), Some("value"));

        store.get_mut("items").unwrap().readonly = true;
        assert_eq!(store.set_indexed_element("items", 1, "blocked"), Err("readonly variable"));
        assert_eq!(store.remove_indexed_element("items", 9), Err("readonly variable"));
    }
}
