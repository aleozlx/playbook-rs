use yaml_rust::{Yaml, YamlLoader, YamlEmitter};
use std::ops::Index;
use rpds::HashTrieMap;
use std::fmt::{Display, Formatter, Result};
use linked_hash_map::LinkedHashMap;

use pyo3::prelude::*;
use pyo3::Python;
use pyo3::types::{PyDict, PyString, PyList};

#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    data: HashTrieMap<String, CtxObj>
}

#[derive(Clone, Debug, PartialEq)]
pub enum CtxObj {
    Str(String),
    Int(i64),
    Real(f64),
    Bool(bool),
    Array(Vec<CtxObj>),
    Context(Context),
    None
}

impl From<Yaml> for CtxObj {
    fn from(src: Yaml) -> CtxObj {
        match src {
            Yaml::String(val) => { CtxObj::Str(val.to_owned()) },
            Yaml::Boolean(val) => { CtxObj::Bool(val.to_owned()) },
            Yaml::Integer(val) => { CtxObj::Int(val.to_owned()) },
            Yaml::Real(val) => { CtxObj::Real(val.parse().unwrap()) }
            Yaml::Null => { CtxObj::None },
            Yaml::Hash(_) => { CtxObj::Context(Context::from(src)) },
            Yaml::Array(val) => {
                CtxObj::Array(val.iter().map(|i| { CtxObj::from(i.clone()) }).collect()) 
            },
            Yaml::Alias(_val) => {
                unimplemented!();
            },
            Yaml::BadValue => { unreachable!(); },
        }
    }
}

impl Into<Yaml> for CtxObj {
    fn into(self) -> Yaml {
        match self {
            CtxObj::Str(val) => Yaml::String(val.to_owned()),
            CtxObj::Bool(val) => Yaml::Boolean(val.to_owned()),
            CtxObj::Int(val) => Yaml::Integer(val.to_owned()),
            CtxObj::Real(val) => Yaml::Real(val.to_string()),
            CtxObj::None => Yaml::Null,
            CtxObj::Context(val) => val.clone().into(),
            CtxObj::Array(val) => Yaml::Array(val.iter().map(|i| {i.clone().into()}).collect())
        }
    }
}

impl ToPyObject for CtxObj {
    fn to_object(&self, py: Python) -> PyObject {
        match self {
            CtxObj::None => py.None(),
            CtxObj::Str(val) => val.to_object(py),
            CtxObj::Bool(val) => val.to_object(py),
            CtxObj::Int(val) => val.to_object(py),
            CtxObj::Real(val) => val.to_object(py),
            CtxObj::Context(val) => val.to_object(py),
            CtxObj::Array(val) => {
                let tmp: Vec<PyObject> = val.iter().map(|i| {i.to_object(py)}).collect();
                PyList::new(py, &tmp).to_object(py)
            }
        }
    }
}

impl From<Yaml> for Context {
    fn from(src: Yaml) -> Self {
        let mut context_data = HashTrieMap::new();
        if let Yaml::Hash(raw) = src {
            for (k, v) in raw {
                if let Yaml::String(key) = k {
                    match v {
                        Yaml::String(_) | Yaml::Boolean(_) | Yaml::Integer(_) | Yaml::Real(_) | Yaml::Null | Yaml::Hash(_) | Yaml::Array(_) | Yaml::Alias(_) => {
                            context_data.insert_mut(key.to_owned(), CtxObj::from(v));
                        }
                        Yaml::BadValue => { }
                    }
                }
            }
        }
        Context { data: context_data }
    }
}

impl<'a> From<&'a str> for Context {
    fn from(s: &str) -> Self {
        Context::from(YamlLoader::load_from_str(s).unwrap()[0].clone())
    }
}

impl Into<Yaml> for Context {
    fn into(self) -> Yaml {
        let mut map = LinkedHashMap::new();
        for (k, v) in self.data.iter() {
            map.insert(Yaml::String(k.to_owned()), v.to_owned().into());
        }
        Yaml::Hash(map)
    }
}

impl ToPyObject for Context {
    fn to_object(&self, py: Python) -> PyObject {
        let ctx = PyDict::new(py);
        for (k, v) in self.data.iter() {
            ctx.set_item(PyString::new(py, k), v.to_object(py)).unwrap();
        }
        ctx.to_object(py)
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut out_str = String::new();
        {
            let mut emitter = YamlEmitter::new(&mut out_str);
            emitter.dump(&self.clone().into()).unwrap();
        }
        write!(f, "{}", &out_str)
    }
}

impl<'a> Index<&'a str> for Context {
    type Output = CtxObj;

    fn index(&self, key: &'a str) -> &CtxObj {
        self.data.get(key).expect(&format!("Key error: {}", key))
    }
}

impl Context {
    pub fn overlay(&self, another: &Context) -> Context {
        let mut forward_snapshot = self.data.clone();
        for (k, v) in another.data.iter() {
            forward_snapshot = forward_snapshot.insert(k.to_owned(), v.to_owned());
        }
        Context { data: forward_snapshot }
    }

    pub fn assign(&self, key: &str, val: CtxObj) -> Context {
        Context { data: self.data.insert(key.to_owned(), val) }
    }

    pub fn subcontext(&self, key: &str) -> Option<Context> {
        if let CtxObj::Context(val) = &self.data[key] { Some(val.clone()) }
        else { None }
    }
}



#[cfg(test)]
mod tests{
    // use yaml_rust::YamlLoader;
    use context::Context;

    #[test]
    fn multiple_overwrites() {
        let a = Context::from("a: 1\nb: 0");
        let b = Context::from("a: 0\nb: 1");
        let c = a.overlay(&b);
        assert_eq!(c, b);
    }

    #[test]
    fn single_overwrite() {
        let a = Context::from("a: 1\nb: 0");
        let b = Context::from("b: 1");
        let c = a.overlay(&b);
        assert_eq!(c, Context::from("a: 1\nb: 1"));
    }

    #[test]
    fn insertion() {
        let a = Context::from("a: 1\nb: 0");
        let b = Context::from("c: 1");
        let c = a.overlay(&b);
        assert_eq!(c, Context::from("a: 1\nb: 0\nc: 1"));
    }
}
