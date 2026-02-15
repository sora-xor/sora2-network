use frame_support::weights::Weight;

pub trait WeightInfo {
    fn add_token() -> Weight;
    fn set_remote_token() -> Weight;
    fn set_domain_endpoint() -> Weight;
    fn clear_domain_endpoint() -> Weight;
    fn set_evm_anchor_mode_enabled() -> Weight;
    fn init_bsc_light_client(validators_len: u32) -> Weight;
    fn submit_bsc_header(header_len: u32) -> Weight;
    fn set_bsc_validators(validators_len: u32) -> Weight;
    fn init_tron_light_client(witnesses_len: u32) -> Weight;
    fn submit_tron_header(raw_len: u32) -> Weight;
    fn set_tron_witnesses(witnesses_len: u32) -> Weight;
    fn set_inbound_attesters(attesters_len: u32) -> Weight;
    fn clear_inbound_attesters() -> Weight;
    fn activate_token() -> Weight;
    fn remove_token() -> Weight;
    fn finalize_remove() -> Weight;
    fn set_inbound_grace_period() -> Weight;
    fn set_required_domains(domains_len: u32) -> Weight;
    fn set_inbound_finality_mode() -> Weight;
    fn set_inbound_domain_paused() -> Weight;
    fn set_outbound_domain_paused() -> Weight;
    fn invalidate_inbound_message() -> Weight;
    fn clear_invalidated_inbound_message() -> Weight;
    fn burn() -> Weight;
    fn mint_from_proof() -> Weight;
    fn attest_burn() -> Weight;
}

impl WeightInfo for () {
    fn add_token() -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn set_remote_token() -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn set_domain_endpoint() -> Weight {
        Weight::from_parts(20_000_000, 0)
    }
    fn clear_domain_endpoint() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn set_evm_anchor_mode_enabled() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn init_bsc_light_client(_validators_len: u32) -> Weight {
        Weight::from_parts(200_000_000, 0)
    }
    fn submit_bsc_header(_header_len: u32) -> Weight {
        Weight::from_parts(300_000_000, 0)
    }
    fn set_bsc_validators(_validators_len: u32) -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn init_tron_light_client(_witnesses_len: u32) -> Weight {
        Weight::from_parts(200_000_000, 0)
    }
    fn submit_tron_header(_raw_len: u32) -> Weight {
        Weight::from_parts(300_000_000, 0)
    }
    fn set_tron_witnesses(_witnesses_len: u32) -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn set_inbound_attesters(_attesters_len: u32) -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn clear_inbound_attesters() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn activate_token() -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn remove_token() -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn finalize_remove() -> Weight {
        Weight::from_parts(50_000_000, 0)
    }
    fn set_inbound_grace_period() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn set_required_domains(_domains_len: u32) -> Weight {
        Weight::from_parts(20_000_000, 0)
    }
    fn set_inbound_finality_mode() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn set_inbound_domain_paused() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn set_outbound_domain_paused() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
    fn invalidate_inbound_message() -> Weight {
        Weight::from_parts(20_000_000, 0)
    }
    fn clear_invalidated_inbound_message() -> Weight {
        Weight::from_parts(20_000_000, 0)
    }
    fn burn() -> Weight {
        Weight::from_parts(100_000_000, 0)
    }
    fn mint_from_proof() -> Weight {
        Weight::from_parts(150_000_000, 0)
    }
    fn attest_burn() -> Weight {
        Weight::from_parts(150_000_000, 0)
    }
}
