use ink::env::{DefaultEnvironment, Environment};

pub enum MockEnvironment {}

impl Environment for MockEnvironment {}
