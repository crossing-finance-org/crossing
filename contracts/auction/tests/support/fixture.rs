use super::*;

pub(super) struct Fixture {
    pub(super) e: Env,
    pub(super) issuer: Address,
    pub(super) quote_admin: Address,
    pub(super) rwa: Address,
    pub(super) quote: Address,
    pub(super) factory: Address,
    pub(super) registry: Address,
    pub(super) template: BytesN<32>,
    pub(super) native_auction: bool,
}
impl Fixture {
    pub(super) fn new() -> Self {
        Self::with_mode(&std::env::var("CROSSING_TEST_MODE").unwrap_or_default())
    }
    pub(super) fn with_mode(test_mode: &str) -> Self {
        let mut e = Env::default();
        e.set_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        e.cost_estimate().budget().reset_unlimited();
        e.ledger().set_sequence_number(90);
        e.ledger().set_timestamp(1_800_000_000);
        let issuer = Address::generate(&e);
        let quote_admin = Address::generate(&e);
        let r = e.register_stellar_asset_contract_v2(issuer.clone());
        let q = e.register_stellar_asset_contract_v2(quote_admin.clone());
        for asset in [&r, &q] {
            for flag in [
                IssuerFlags::RequiredFlag,
                IssuerFlags::RevocableFlag,
                IssuerFlags::ClawbackEnabledFlag,
            ] {
                asset.issuer().set_flag(flag);
            }
        }
        let rwa = r.address();
        let quote = q.address();
        assert!(matches!(test_mode, "" | "services" | "auction"));
        let native_auction = test_mode == "auction";
        let native_services = !test_mode.is_empty();
        let native_hash: BytesN<32> = e.crypto().sha256(&soroban_sdk::Bytes::new(&e)).into();
        let template = if native_auction {
            native_hash.clone()
        } else {
            e.deployer().upload_contract_wasm(wasm("crossing_auction"))
        };
        let fh = if native_services {
            native_hash.clone()
        } else {
            e.deployer()
                .upload_contract_wasm(wasm("crossing_auction_factory"))
        };
        let rh = if native_services {
            native_hash
        } else {
            e.deployer()
                .upload_contract_wasm(wasm("crossing_price_registry"))
        };
        let factory = Address::generate(&e);
        let registry_args = (factory.clone(), fh, template.clone());
        let registry = if native_services {
            e.register(crossing_price_registry::PriceRegistry, registry_args)
        } else {
            e.register(wasm("crossing_price_registry"), registry_args)
        };
        let factory_args = (registry.clone(), rh, template.clone());
        if native_auction {
            e.register_at(&factory, NativeAuctionFactory, factory_args);
        } else if native_services {
            e.register_at(
                &factory,
                crossing_auction_factory::AuctionFactory,
                factory_args,
            );
        } else {
            e.register_at(&factory, wasm("crossing_auction_factory"), factory_args);
        }
        let f = Self {
            e,
            issuer,
            quote_admin,
            rwa,
            quote,
            factory,
            registry,
            template,
            native_auction,
        };
        f.set_auth(&f.issuer, &f.rwa, true);
        f.set_auth(&f.issuer, &f.quote, true);
        f.mint(&f.issuer, &f.rwa, 1_000_000);
        f.mint(&f.issuer, &f.quote, 1_000_000);
        f
    }
    pub(super) fn admin(&self, asset: &Address) -> &Address {
        if asset == &self.rwa {
            &self.issuer
        } else {
            &self.quote_admin
        }
    }
    pub(super) fn signed(
        &self,
        owner: &Address,
        contract: &Address,
        function: &str,
        args: Vec<Val>,
        children: &[MockAuthInvoke],
    ) {
        self.e.mock_auths(&[MockAuth {
            address: owner,
            invoke: &MockAuthInvoke {
                contract,
                fn_name: function,
                args,
                sub_invokes: children,
            },
        }]);
    }
    pub(super) fn clear_auth(&self) {
        self.e.set_auths(&[]);
    }
    pub(super) fn set_auth(&self, owner: &Address, asset: &Address, allowed: bool) {
        self.signed(
            self.admin(asset),
            asset,
            "set_authorized",
            (owner.clone(), allowed).into_val(&self.e),
            &[],
        );
        token::StellarAssetClient::new(&self.e, asset).set_authorized(owner, &allowed);
        self.clear_auth();
    }
    pub(super) fn mint(&self, owner: &Address, asset: &Address, amount: i128) {
        self.signed(
            self.admin(asset),
            asset,
            "mint",
            (owner.clone(), amount).into_val(&self.e),
            &[],
        );
        token::StellarAssetClient::new(&self.e, asset).mint(owner, &amount);
        self.clear_auth();
    }
    pub(super) fn clawback(&self, owner: &Address, asset: &Address, amount: i128) {
        self.signed(
            self.admin(asset),
            asset,
            "clawback",
            (owner.clone(), amount).into_val(&self.e),
            &[],
        );
        token::StellarAssetClient::new(&self.e, asset).clawback(owner, &amount);
        self.clear_auth();
    }
    pub(super) fn user(&self) -> Address {
        let user = Address::generate(&self.e);
        self.set_auth(&user, &self.rwa, true);
        self.set_auth(&user, &self.quote, true);
        self.mint(&user, &self.rwa, 1_000_000);
        self.mint(&user, &self.quote, 1_000_000);
        user
    }
    pub(super) fn config(&self, mode: Mode, sealed: bool, supply: i128) -> Config {
        Config {
            version: 1,
            rwa_token: self.rwa.clone(),
            quote_token: self.quote.clone(),
            issuer: self.issuer.clone(),
            buyback_recipient: self.issuer.clone(),
            mode,
            bid_mode: if sealed {
                BidMode::Sealed
            } else {
                BidMode::Open
            },
            nonreveal_penalty_bps: 0,
            supply_or_budget: supply,
            min_fill_quantity: 1,
            min_bid_quantity: 1,
            min_deposit: 1,
            min_price: cc::SCALE,
            max_price: 20 * cc::SCALE,
            tick_size: cc::SCALE,
            max_bids: cc::MAX_BIDS,
            max_bids_per_address: cc::MAX_BIDS,
            start_ledger: 100,
            end_ledger: 110,
            reveal_end_ledger: if sealed { 115 } else { 110 },
            close_deadline: 140,
            delivery_deadline: 160,
        }
    }
    pub(super) fn create(&self, c: &Config, salt: u8) -> Address {
        let salt = BytesN::from_array(&self.e, &[salt; 32]);
        self.signed(
            &self.issuer,
            &self.factory,
            "create",
            (c.clone(), salt.clone()).into_val(&self.e),
            &[],
        );
        let a: Address = self.e.invoke_contract(
            &self.factory,
            &Symbol::new(&self.e, "create"),
            (c.clone(), salt).into_val(&self.e),
        );
        self.clear_auth();
        a
    }
    pub(super) fn fund(&self, a: &Address, c: &Config) {
        self.set_auth(a, &self.rwa, true);
        self.set_auth(a, &self.quote, true);
        let source = if c.mode == Mode::Sale {
            &self.rwa
        } else {
            &self.quote
        };
        self.signed(
            &self.issuer,
            a,
            "fund",
            ().into_val(&self.e),
            &[MockAuthInvoke {
                contract: source,
                fn_name: "transfer",
                args: (self.issuer.clone(), a.clone(), c.supply_or_budget).into_val(&self.e),
                sub_invokes: &[],
            }],
        );
        AuctionClient::new(&self.e, a).fund();
        self.clear_auth();
    }
    pub(super) fn auction(&self, c: &Config, salt: u8) -> Address {
        let a = self.create(c, salt);
        self.fund(&a, c);
        a
    }
    pub(super) fn place(&self, a: &Address, user: &Address, p: i128, q: i128) -> u32 {
        self.place_raw(a, user, p * cc::SCALE, q)
    }
    pub(super) fn place_raw(&self, a: &Address, user: &Address, price: i128, q: i128) -> u32 {
        let c = AuctionClient::new(&self.e, a).config();
        let (asset, deposit) = if c.mode == Mode::Sale {
            (&self.quote, cc::cost(&self.e, c.mode, q, price).unwrap())
        } else {
            (&self.rwa, q)
        };
        self.signed(
            user,
            a,
            "place_bid",
            (user.clone(), price, q).into_val(&self.e),
            &[MockAuthInvoke {
                contract: asset,
                fn_name: "transfer",
                args: (user.clone(), a.clone(), deposit).into_val(&self.e),
                sub_invokes: &[],
            }],
        );
        let id = AuctionClient::new(&self.e, a).place_bid(user, &price, &q);
        self.clear_auth();
        id
    }
    pub(super) fn commit(
        &self,
        a: &Address,
        user: &Address,
        price: i128,
        quantity: i128,
        locked: i128,
    ) -> (u32, BytesN<32>) {
        let client = AuctionClient::new(&self.e, a);
        let seed = client.state().bid_count as u8; // Deterministic test secrets only.
        let nonce = BytesN::from_array(&self.e, &[seed; 32]);
        let salt = BytesN::from_array(&self.e, &[seed.wrapping_add(1); 32]);
        let preimage = (
            Symbol::new(&self.e, "CROSSING_BID_V1"),
            self.e.ledger().network_id(),
            a.clone(),
            user.clone(),
            nonce.clone(),
            client.provenance().config_hash,
            locked,
            price,
            quantity,
            salt.clone(),
        )
            .to_xdr(&self.e);
        let hash: BytesN<32> = self.e.crypto().sha256(&preimage).into();
        (self.submit_commit(a, user, &nonce, &hash, locked), salt)
    }
    pub(super) fn submit_commit(
        &self,
        a: &Address,
        user: &Address,
        nonce: &BytesN<32>,
        hash: &BytesN<32>,
        locked: i128,
    ) -> u32 {
        let client = AuctionClient::new(&self.e, a);
        let asset = if client.config().mode == Mode::Sale {
            &self.quote
        } else {
            &self.rwa
        };
        self.signed(
            user,
            a,
            "commit_bid",
            (user.clone(), nonce.clone(), hash.clone(), locked).into_val(&self.e),
            &[MockAuthInvoke {
                contract: asset,
                fn_name: "transfer",
                args: (user.clone(), a.clone(), locked).into_val(&self.e),
                sub_invokes: &[],
            }],
        );
        let id = client.commit_bid(user, nonce, hash, &locked);
        self.clear_auth();
        id
    }
    pub(super) fn reveal(
        &self,
        a: &Address,
        id: u32,
        price: i128,
        quantity: i128,
        salt: &BytesN<32>,
    ) {
        let client = AuctionClient::new(&self.e, a);
        let owner = client.get_bid(&id).owner;
        self.signed(
            &owner,
            a,
            "reveal_bid",
            (id, price, quantity, salt.clone()).into_val(&self.e),
            &[],
        );
        client.reveal_bid(&id, &price, &quantity, salt);
        self.clear_auth();
    }
    pub(super) fn ready(&self, a: &Address) {
        let client = AuctionClient::new(&self.e, a);
        for _ in 0..256 {
            match client.state().stage {
                Stage::Ready | Stage::Cancelled | Stage::Refunded => return,
                _ => {
                    client.advance(&32);
                }
            }
        }
        panic!("progress bound");
    }
    pub(super) fn close(&self, a: &Address) {
        let client = AuctionClient::new(&self.e, a);
        for _ in 0..33 {
            self.ready(a);
            match client.state().stage {
                Stage::Cancelled | Stage::Refunded => return,
                Stage::Ready => {
                    if !matches!(client.finalize(), Finalization::Excluded(_, _)) {
                        return;
                    }
                }
                _ => panic!("stage"),
            }
        }
        panic!("restart bound");
    }
    pub(super) fn withdraw(&self, a: &Address, owner: &Address, asset: &Address, amount: i128) {
        self.signed(
            owner,
            a,
            "withdraw",
            (owner.clone(), asset.clone(), amount).into_val(&self.e),
            &[],
        );
        AuctionClient::new(&self.e, a).withdraw(owner, asset, &amount);
        self.clear_auth();
    }
    pub(super) fn check(&self, a: &Address) {
        let client = AuctionClient::new(&self.e, a);
        let accounting = client.accounting();
        assert!(accounting.balanced());
        let (r, rl, q, ql) = client.solvency();
        assert!(r >= rl && q >= ql);
    }
}

#[contract]
pub(super) struct Fake;
#[contractimpl]
impl Fake {
    pub fn authorized(_env: Env, _id: Address) -> bool {
        true
    }
}
