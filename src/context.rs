use yaml_rust::Yaml;
use std::ops::Index;
use rpds::HashTrieMap;
use std::fmt::{Display, Formatter, Result};

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
    Array(Vec<Context>),
    Context(Context),
    None
}

impl<'a> From<&'a Yaml> for Context {
    fn from(src: &'a Yaml) -> Self {
        let mut context_data = HashTrieMap::new();
        if let Yaml::Hash(raw) = src {
            for (k, v) in raw {
                if let Yaml::String(key) = k {
                    match v {
                        Yaml::String(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Str(val.to_owned()));
                        },
                        Yaml::Boolean(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Bool(val.to_owned()));
                        },
                        Yaml::Integer(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Int(val.to_owned()));
                        },
                        Yaml::Real(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Real(val.parse().unwrap()));
                        },
                        Yaml::Null => { context_data.insert_mut(
                            key.to_owned(), CtxObj::None);
                        },
                        Yaml::Hash(_) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Context(Context::from(v))); 
                        },
                        Yaml::Array(val) => {
                            let vv: Vec<Context> = val.iter().map(|i| {Context::from(i)}).collect();
                            context_data.insert_mut(key.to_owned(), CtxObj::Array(vv)); 
                        },
                        Yaml::Alias(_val) => {
                            unimplemented!();
                        },
                        Yaml::BadValue => { }
                    }
                }
            }
        }
        Context { data: context_data }
    }
}

impl Into<String> for Context {
    fn into(self) -> String {
        String::from("hi")
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "(hi)")
    }
}

impl<'a> Index<&'a str> for Context {
    type Output = CtxObj;

    fn index(&self, key: &'a str) -> &CtxObj {
        self.data.get(key).expect("no entry found for key")
    }
}

impl Context {
    fn overlay(&self, another: &Context) -> Context {
        let mut ret = self.data.clone();
        for (k, v) in another.data.iter() {
            ret = ret.insert(k.to_owned(), v.to_owned());
        }
        Context { data: ret }
    }
}

#[test]
fn test1() {
    let a = Context::from(&Yaml::from_str("a: 1\nb: 0"));
    let b = Context::from(&Yaml::from_str("a: 0\nb: 1"));
    let c = a.overlay(&b);
    assert_eq!(c, Context::from(&Yaml::from_str("a: 1\nb: 0")));
}