//! Simulation inspired by CircuitVerse, UI based off of KiCad

use std::{marker::PhantomData, ops};
use serde::{Serialize, Deserialize};
use nalgebra::Vector2;

pub mod simulator;
pub mod ui;
pub mod resource_interface;
pub mod basic_components;
#[cfg(test)]
pub mod tests;

#[allow(unused)]
pub mod prelude {
	use std::{clone, fmt::Formatter};
    use super::*;
	// Name of this app
	pub const APP_NAME: &str = "Rusty Logic";
    pub type V2 = Vector2<f32>;
    pub use ui::{Styles, LogicCircuitToplevelView, App};
    pub use simulator::{LogicDevice, LogicDeviceGeneric, Wire, LogicNet, LogicConnectionPin, LogicCircuit};
    pub use resource_interface::{load_file_with_better_error, EnumAllLogicDevicesSave};
    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    pub enum FourWayDir {
        E,
        N,
        W,
        S
    }
    impl FourWayDir {
        pub fn to_unit(&self) -> V2 {
            match &self {
                Self::E => V2::new(1.0, 0.0),
                Self::N => V2::new(0.0, 1.0),
                Self::W => V2::new(-1.0, 0.0),
                Self::S => V2::new(0.0, -1.0)
            }
        }
        pub fn to_dir_deg(&self) -> f32 {
            match &self {
                Self::E => 0.0,
                Self::N => 90.0,
                Self::W => 180.0,
                Self::S => 270.0
            }
        }
        pub fn to_string(&self) -> String {
            match &self {
                Self::E => "East".to_string(),
                Self::N => "North".to_string(),
                Self::W => "West".to_string(),
                Self::S => "South".to_string()
            }
        }
    }
    impl Default for FourWayDir {
        fn default() -> Self {
            Self::E
        }
    }
    pub fn to_string_err<T, E: ToString>(result: Result<T, E>) -> Result<T, String> {
		match result {
			Ok(t) => Ok(t),
			Err(e) => Err(e.to_string())
		}
	}
	pub fn to_string_err_with_message<T, E: ToString>(result: Result<T, E>, message: &str) -> Result<T, String> {
		match result {
			Ok(t) => Ok(t),
			Err(e) => Err(format!("Message: {}, Error: {}", message, e.to_string()))
		}
	}
    /// Generic reference to things in the game
    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
    pub struct GenericRef<T> {
        pub id: u64,
        pub unique_name_opt: Option<String>,
        #[serde(skip)]
        _phantom: PhantomData<T>
    }

    impl<T> GenericRef<T> {
        pub fn new(id: u64, unique_name_opt: Option<String>) -> Self {
            Self {
                id,
                unique_name_opt,
                _phantom: PhantomData{}
            }
        }
        pub fn id(id: u64) -> Self {
            Self {
                id,
                unique_name_opt: None,
                _phantom: PhantomData{}
            }
        }
        pub fn to_query(&self) -> GenericQuery<T> {
            GenericQuery::id(self.id)
        }
        pub fn query_matches(&self, query: &GenericQuery<T>) -> bool {
            match query {
                GenericQuery::Id(id, _) => self.id == *id,
                GenericQuery::UniqueName(other_name, _) => match &self.unique_name_opt {
                    Some(self_name) => other_name == self_name,
                    None => false
                }
            }
        }
        /// WARNING: This method can do what this whole type is meant to avoid: using references in the wrong context. Use with caution.
        pub fn into_another_type<T2>(&self) -> GenericRef<T2> {
            GenericRef::<T2> {
                id: self.id,
                unique_name_opt: self.unique_name_opt.clone(),
                _phantom: PhantomData{}
            }
        }
        pub fn to_string(&self) -> String {
            match &self.unique_name_opt {
                Some(name) => format!("{} ({})", self.id, name),
                None => self.id.to_string()
            }
        }
    }

    impl<T> Clone for GenericRef<T> {
        fn clone(&self) -> Self {
            return Self {
                id: self.id,
                unique_name_opt: self.unique_name_opt.clone(),
                _phantom: PhantomData{}
            }
        }
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    pub struct CustomPhantomData<T> {
        _phantom: PhantomData<T>
    }

    impl<T> CustomPhantomData<T> {
        pub fn new() -> Self {
            Self{_phantom: PhantomData{}}
        }
    }

    impl<T> Default for CustomPhantomData<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum GenericQuery<T> {
        Id(
            u64,
            #[serde(skip)]
            CustomPhantomData<T>
        ),
        UniqueName(
            String,
            #[serde(skip)]
            CustomPhantomData<T>
        )
    }

    impl<T> GenericQuery<T> {
        pub fn id(id: u64) -> Self {
            Self::Id(id, CustomPhantomData::new())
        }
        pub fn unique_name(unique_name: String) -> Self {
            Self::UniqueName(unique_name, CustomPhantomData::new())
        }
        /// WARNING: This method can do what this whole type is meant to avoid: using queries in the wrong context. Use with caution.
        pub fn into_another_type<T2>(&self) -> GenericQuery<T2> {
            match &self {
                Self::Id(id, _) => GenericQuery::<T2>::Id(*id, CustomPhantomData::<T2>::new()),
                Self::UniqueName(name, _) => GenericQuery::<T2>::UniqueName(name.clone(), CustomPhantomData::<T2>::new())
            }
        }
    }

    impl<T> From<&str> for GenericQuery<T> {
        fn from(value: &str) -> GenericQuery<T> {
            GenericQuery::<T>::UniqueName(value.to_string(), CustomPhantomData::new())
        }
    }

    impl<T> From<u64> for GenericQuery<T> {
        fn from(value: u64) -> GenericQuery<T> {
            GenericQuery::<T>::id(value)
        }
    }

    impl<T> Clone for GenericQuery<T> {
        fn clone(&self) -> Self {
            match &self {
                Self::Id(id, phantom) => Self::Id(*id, CustomPhantomData::new()),
                Self::UniqueName(name, phantom) => Self::UniqueName(name.clone(), CustomPhantomData::new())
            }
        }
    }

    impl<T> PartialEq for GenericQuery<T> {
        fn eq(&self, other: &Self) -> bool {
            match self {
                Self::Id(self_id, _) => if let Self::Id(other_id, _) = other {
                    self_id == other_id
                }
                else {
                    false
                },
                Self::UniqueName(self_id, _) => if let Self::UniqueName(other_id, _) = other {
                    self_id == other_id
                }
                else {
                    false
                }
            }
        }
    }

    impl<T> Default for GenericQuery<T> {
        fn default() -> Self {
            Self::id(0)
        }
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct GenericDataset<T> {
        pub items: Vec<(GenericRef<T>, T)>
    }

    impl<T> GenericDataset<T> {
        pub fn new() -> Self {
            Self {
                items: Vec::new()
            }
        }
        pub fn get_item_index_with_query(&self, query: &GenericQuery<T>) -> Option<usize> {
            for (i, (ref_, _)) in self.items.iter().enumerate() {
                if ref_.query_matches(query) {
                    return Some(i)
                }
            }
            // Default
            None
        }
        pub fn get_item_tuple(&self, query: &GenericQuery<T>) -> Option<(&GenericRef<T>, &T)> {
            match self.get_item_index_with_query(query) {
                Some(i) => Some((&self.items[i].0, &self.items[i].1)),
                None => None
            }
        }
        pub fn get_item_mut(&mut self, query: &GenericQuery<T>) -> Option<&mut T> {
            match self.get_item_index_with_query(query) {
                Some(i) => Some(&mut self.items[i].1),
                None => None
            }
        }
        pub fn get_item_id(&self, query: &GenericQuery<T>) -> Option<u64> {
            match self.get_item_tuple(query) {
                Some((ref_, _)) => Some(ref_.id),
                None => None
            }
        }
    }
    
    impl<T> From<Vec<T>> for GenericDataset<T> {
        fn from(value: Vec<T>) -> Self {
            let mut items = Vec::<(GenericRef<T>, T)>::new();
            for (i, item) in value.into_iter().enumerate() {
                items.push((GenericRef::id(i as u64), item));
            }
            // Done
            Self {
                items
            }
        }
    }

    impl<T> From<Vec<(T, &str)>> for GenericDataset<T> {
        fn from(value: Vec<(T, &str)>) -> Self {
            let mut items = Vec::<(GenericRef<T>, T)>::new();
            for (i, (item, name)) in value.into_iter().enumerate() {
                items.push((GenericRef::new(i as u64, Some(name.to_string())), item));
            }
            // Done
            Self {
                items
            }
        }
    }

    #[derive(Default, Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Eq, Hash)]
    pub struct IntV2(pub i32, pub i32);

    impl IntV2 {
        pub fn mult(&self, other: i32) -> Self {
            Self(self.0 * other, self.1 * other)
        }
        pub fn to_v2(&self) -> V2 {
            V2::new(self.0 as f32, self.1 as f32)
        }
    }

    impl ops::Add<IntV2> for IntV2 {
        type Output = IntV2;

        fn add(self, other: Self) -> Self {
            Self(self.0 + other.0, self.1 + other.1)
        }
    }

    impl ops::Sub<IntV2> for IntV2 {
        type Output = IntV2;

        fn sub(self, other: IntV2) -> Self {
            Self(self.0 - other.0, self.1 - other.1)
        }
    }

    impl ops::Index<usize> for IntV2 {
        type Output = i32;
        fn index(&self, index: usize) -> &Self::Output {
            match index {
                0 => &self.0,
                1 => &self.1,
                n => panic!("IntV2 index must be 0 or 1, not {}", n)
            }
        }
    }
}

use prelude::*;

fn main() {
	let native_options = eframe::NativeOptions::default();
	eframe::run_native(&APP_NAME, native_options, Box::new(|_| Ok(Box::new(App::new())))).unwrap();
}