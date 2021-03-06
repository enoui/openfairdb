// Copyright (c) 2015 - 2018 Markus Kohlhase <mail@markus-kohlhase.de>
// Copyright (c) 2018 - 2020 slowtec GmbH <post@slowtec.de>

#![feature(plugin, test, proc_macro_hygiene, decl_macro, never_type)]
#![allow(proc_macro_derive_resolution_fallback)]
#![recursion_limit = "128"]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde;
#[cfg(test)]
extern crate test;

mod adapters;
mod core;
pub(crate) mod infrastructure;
mod ports;

fn main() {
    env_logger::init();
    ports::cli::run();
}
