//! API de consultation des emplois du temps du département informatique de
//! l'IUT du Limousin.
//!
//! Le département publie un PDF par année et par semaine. Le service les
//! télécharge, en extrait les créneaux ([`pdf`]) et les sert en JSON ([`http`]).

pub mod catalog;
pub mod config;
pub mod discord;
pub mod error;
pub mod http;
pub mod ics;
pub mod model;
pub mod notify;
pub mod pdf;
pub mod session;
pub mod store;
pub mod upstream;
pub mod users;
