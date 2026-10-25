//! All-contract WASM behavior and native coverage scenarios using genuine SACs.
#[path = "scenarios/accounting.rs"]
mod accounting;
#[path = "support/artifacts.rs"]
mod artifacts;
#[path = "scenarios/authorization.rs"]
mod authorization;
#[path = "scenarios/classic_authority_tests.rs"]
mod classic_authority_tests;
#[path = "scenarios/clearing.rs"]
mod clearing;
#[path = "scenarios/commitment.rs"]
mod commitment;
#[path = "scenarios/cryptographic_tests.rs"]
mod cryptographic_tests;
#[path = "scenarios/demo.rs"]
mod demo;
#[path = "scenarios/factory.rs"]
mod factory;
#[path = "support/factory_bootstrap.rs"]
mod factory_bootstrap;
#[path = "support/fixture.rs"]
mod fixture;
#[path = "scenarios/lifecycle.rs"]
mod lifecycle;
#[path = "support/native_adapter.rs"]
mod native_adapter;
#[path = "scenarios/oracle_audit_tests.rs"]
mod oracle_audit_tests;
#[path = "scenarios/penalty_audit_tests.rs"]
mod penalty_audit_tests;
#[path = "scenarios/registry.rs"]
mod registry;
#[path = "scenarios/resource_audit_tests.rs"]
mod resource_audit_tests;
#[path = "scenarios/runtime_gap_tests.rs"]
mod runtime_gap_tests;
#[path = "scenarios/vectors.rs"]
mod vectors;
use crossing_auction::AuctionClient;
use crossing_common::{self as cc, BidMode, Config, Error, Finalization, Mode, Observation, Stage};
use native_adapter::NativeAuctionFactory;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{
        Address as _, Deployer as _, EnvTestConfig, IssuerFlags, Ledger as _, MockAuth,
        MockAuthInvoke,
    },
    token,
    xdr::ToXdr,
    Address, BytesN, Env, IntoVal, Symbol, TryIntoVal, Val, Vec,
};

use artifacts::wasm;
use fixture::*;
