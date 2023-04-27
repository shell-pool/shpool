use crate::Error;
use serde::{Serialize, Deserialize};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Inheritable<T> {
    Set(T),
    Inherited { workspace: bool },
}

impl<T> Inheritable<T> {
    pub fn as_ref(&self) -> Inheritable<&T> {
        match self {
            Self::Set(t) => Inheritable::Set(t),
            Self::Inherited{..} => Inheritable::Inherited{workspace:true},
        }
    }

    pub fn is_set(&self) -> bool {
        matches!(self, Self::Set(_))
    }

    pub fn get(&self) -> Result<&T, Error> {
        match self {
            Self::Set(t) => Ok(t),
            Self::Inherited{..} => Err(Error::InheritedUnknownValue),
        }
    }

    pub fn set(&mut self, val: T) {
        *self = Self::Set(val)
    }

    pub fn as_mut(&mut self) -> Inheritable<&mut T> {
        match self {
            Self::Set(t) => Inheritable::Set(t),
            Self::Inherited{..} => Inheritable::Inherited{workspace:true},
        }
    }

    pub fn get_mut(&mut self) -> Result<&mut T, Error> {
        match self {
            Self::Set(t) => Ok(t),
            Self::Inherited{..} => Err(Error::InheritedUnknownValue),
        }
    }

    #[track_caller]
    pub fn unwrap(self) -> T {
        match self {
            Self::Set(t) => t,
            Self::Inherited{..} => panic!("inherited workspace value"),
        }
    }

    pub fn inherit(&mut self, other: &T) where T: Clone {
        if let Self::Inherited{..} = self {
            *self = Self::Set(other.clone())
        }
    }
}

impl<T: Default> Default for Inheritable<T> {
    fn default() -> Self {
        Self::Set(T::default())
    }
}

impl<T> Inheritable<Vec<T>> {
    #[must_use] pub fn is_empty(&self) -> bool {
        match self {
            Self::Inherited{..} => false,
            Self::Set(v) => v.is_empty(),
        }
    }
}

impl<T: Default + PartialEq> Inheritable<T> {
    pub fn is_default(&self) -> bool {
        match self {
            Self::Inherited{..} => false,
            Self::Set(v) => T::default() == *v,
        }
    }
}

impl<T> From<Option<T>> for Inheritable<T> {
    fn from(val: Option<T>) -> Self {
        match val {
            Some(val) => Self::Set(val),
            None => Self::Inherited{workspace:true},
        }
    }
}

impl<T> From<Inheritable<T>> for Option<T> {
    fn from(val: Inheritable<T>) -> Self {
        match val {
            Inheritable::Inherited{..} => None,
            Inheritable::Set(val) => Some(val),
        }
    }
}
