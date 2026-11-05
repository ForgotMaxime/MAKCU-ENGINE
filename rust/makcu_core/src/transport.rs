use crate::Result;

pub trait Transport {
    fn query(&self, name: &str, args: &[&str]) -> Result<String>;
    fn set(&self, name: &str, args: &[&str]) -> Result<()>;
    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()>;
}

impl<T: Transport> Transport for &T {
    fn query(&self, name: &str, args: &[&str]) -> Result<String> {
        (**self).query(name, args)
    }
    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        (**self).set(name, args)
    }
    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()> {
        (**self).fire_and_forget(name, args)
    }
}
