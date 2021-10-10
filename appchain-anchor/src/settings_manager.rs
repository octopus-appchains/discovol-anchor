use crate::*;

impl Default for ProtocolSettings {
    fn default() -> Self {
        Self {
            minimum_validator_deposit: U128::from(10_000 * OCT_DECIMALS_VALUE),
            minimum_delegator_deposit: U128::from(1000 * OCT_DECIMALS_VALUE),
            minimum_total_stake_for_booting: U128::from(500_000 * OCT_DECIMALS_VALUE),
            maximum_market_value_percent_of_near_fungible_tokens: 33,
            maximum_market_value_percent_of_wrapped_appchain_token: 67,
            minimum_validator_count: U64::from(13),
            maximum_validators_per_delegator: U64::from(16),
            unlock_period_of_validator_deposit: U64::from(21),
            unlock_period_of_delegator_deposit: U64::from(7),
            maximum_era_count_of_unwithdrawn_reward: U64::from(84),
            delegation_fee_percent: 20,
        }
    }
}

pub trait ProtocolSettingsManager {
    ///
    fn change_minimum_validator_deposit(&mut self, value: Balance);
    ///
    fn change_minimum_delegator_deposit(&mut self, value: Balance);
    ///
    fn change_minimum_total_stake_for_booting(&mut self, value: Balance);
    ///
    fn change_maximum_market_value_percent_of_near_fungible_tokens(&mut self, value: u16);
    ///
    fn change_maximum_market_value_percent_of_wrapped_appchain_token(&mut self, value: u16);
    ///
    fn change_minimum_validator_count(&mut self, value: u16);
    ///
    fn change_maximum_validators_per_delegator(&mut self, value: u16);
    ///
    fn change_unlock_period_of_validator_deposit(&mut self, value: u16);
    ///
    fn change_unlock_period_of_delegator_deposit(&mut self, value: u16);
    ///
    fn change_maximum_era_count_of_unwithdrawn_reward(&mut self, value: u16);
}

pub trait AppchainSettingsManager {
    ///
    fn set_chain_spec(&mut self, chain_spec: String);
    ///
    fn set_raw_chain_spec(&mut self, raw_chain_spec: String);
    ///
    fn set_boot_nodes(&mut self, boot_nodes: String);
    ///
    fn set_rpc_endpoint(&mut self, rpc_endpoint: String);
    ///
    fn set_era_reward(&mut self, era_reward: Balance);
}

pub trait AnchorSettingsManager {
    ///
    fn set_token_price_maintainer_account(&mut self, account_id: AccountId);
}
