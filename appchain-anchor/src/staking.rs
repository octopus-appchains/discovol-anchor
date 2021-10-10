use crate::*;
use near_sdk::serde_json;
use validator_set::ValidatorSetActions;

#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct UnbondedStakeReference {
    /// The number of era in appchain.
    pub era_number: u64,
    /// The index of corresponding `staking history`
    pub staking_history_index: u64,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingHistories {
    /// The staking history data happened in this contract.
    histories: LookupMap<u64, StakingHistory>,
    /// The start index of valid staking history.
    start_index: u64,
    /// The end index of valid staking history.
    end_index: u64,
}

impl StakingHistories {
    ///
    pub fn new() -> Self {
        Self {
            histories: LookupMap::new(StorageKey::StakingHistoriesMap.into_bytes()),
            start_index: 0,
            end_index: 0,
        }
    }
    ///
    pub fn get(&self, index: &u64) -> Option<StakingHistory> {
        self.histories.get(index)
    }
    ///
    pub fn index_range(&self) -> IndexRange {
        IndexRange {
            start_index: U64::from(self.start_index),
            end_index: U64::from(self.end_index),
        }
    }
    ///
    pub fn append(&mut self, staking_fact: StakingFact) -> StakingHistory {
        let index = match self.histories.contains_key(&0) {
            true => self.end_index + 1,
            false => 0,
        };
        self.histories.insert(
            &index,
            &StakingHistory {
                staking_fact,
                block_height: env::block_index(),
                timestamp: env::block_timestamp(),
                index: U64::from(index),
            },
        );
        self.end_index = index;
        self.histories.get(&index).unwrap()
    }
}

pub trait StakingManager {
    /// Decrease stake of an account (validator).
    /// This function can only be called by a validator.
    fn decrease_stake(&mut self, amount: U128);
    /// Unbond stake of an account (validator).
    /// This function can only be called by a validator.
    fn unbond_stake(&mut self);
    /// Enable delegation for an account (validator).
    /// This function can only be called by a validator.
    fn enable_delegation(&mut self);
    /// Disable delegation for an account (validator).
    /// This function can only be called by a validator.
    fn disable_delegation(&mut self);
    /// Decrease delegation of an account (delegator) to a validator.
    /// This function can only be called by a delegator.
    fn decrease_delegation(&mut self, validator_id: AccountId, amount: U128);
    /// Unbond delegation of an account (delegator) to a validator.
    /// This function can only be called by a delegator.
    fn unbond_delegation(&mut self, validator_id: AccountId);
    /// Withdraw unbonded stake(s) of a certain account.
    /// This function can be called by any account.
    fn withdraw_stake(&mut self, account_id: AccountId);
    /// Withdraw rewards of a certain validator.
    /// This function can be called by any account.
    fn withdraw_validator_rewards(&mut self, validator_id: AccountId);
    /// Withdraw rewards of a certain delegator to a validator.
    /// This function can be called by any account.
    fn withdraw_delegator_rewards(&mut self, delegator_id: AccountId, validator_id: AccountId);
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
enum StakingDepositMessage {
    RegisterValidator {
        validator_id_in_appchain: AccountIdInAppchain,
        can_be_delegated_to: bool,
    },
    IncreaseStake,
    RegisterDelegator {
        validator_id: AccountId,
    },
    IncreaseDelegation {
        validator_id: AccountId,
    },
}

impl AppchainAnchor {
    //
    pub fn process_oct_deposit(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let deposit_message: StakingDepositMessage = match serde_json::from_str(msg.as_str()) {
            Ok(msg) => msg,
            Err(_) => {
                log!(
                    "Invalid msg '{}' attached in `ft_transfer_call`. Return deposit.",
                    msg
                );
                return PromiseOrValue::Value(amount);
            }
        };
        match deposit_message {
            StakingDepositMessage::RegisterValidator {
                validator_id_in_appchain,
                can_be_delegated_to,
            } => {
                self.register_validator(
                    sender_id,
                    validator_id_in_appchain,
                    amount,
                    can_be_delegated_to,
                );
                PromiseOrValue::Value(0.into())
            }
            StakingDepositMessage::IncreaseStake => {
                self.increase_stake(sender_id, amount);
                PromiseOrValue::Value(0.into())
            }
            StakingDepositMessage::RegisterDelegator { validator_id } => {
                self.register_delegator(sender_id, validator_id, amount);
                PromiseOrValue::Value(0.into())
            }
            StakingDepositMessage::IncreaseDelegation { validator_id } => {
                self.increase_delegation(sender_id, validator_id, amount);
                PromiseOrValue::Value(0.into())
            }
        }
    }
    //
    fn register_validator(
        &mut self,
        validator_id: AccountId,
        validator_id_in_appchain: AccountIdInAppchain,
        deposit_amount: U128,
        can_be_delegated_to: bool,
    ) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        assert!(
            !next_validator_set.validator_ids.contains(&validator_id),
            "The account {} is already been registered.",
            &validator_id
        );
        assert!(
            !self.unbonded_stakes.contains_key(&validator_id),
            "The account {} is holding unbonded stake(s) which need to be withdrawn first.",
            &validator_id
        );
        assert!(
            !self
                .validator_account_id_mapping
                .contains_key(&validator_id_in_appchain),
            "The account {} in appchain is already been registered.",
            &validator_id_in_appchain
        );
        let protocol_settings = self.protocol_settings.get().unwrap();
        assert!(
            deposit_amount.0 >= protocol_settings.minimum_validator_deposit.0,
            "The deposit for registering validator is too few."
        );
        self.record_staking_fact(
            StakingFact::ValidatorRegistered {
                validator_id: validator_id.clone(),
                validator_id_in_appchain: validator_id_in_appchain.clone(),
                amount: deposit_amount,
                can_be_delegated_to,
            },
            &mut next_validator_set,
        );
        self.validator_account_id_mapping
            .insert(&validator_id_in_appchain, &validator_id);
    }
    //
    fn increase_stake(&mut self, validator_id: AccountId, amount: U128) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        self.assert_validator_id(&validator_id, &next_validator_set);
        self.record_staking_fact(
            StakingFact::StakeIncreased {
                validator_id,
                amount,
            },
            &mut next_validator_set,
        );
    }
    //
    fn register_delegator(
        &mut self,
        delegator_id: AccountId,
        validator_id: AccountId,
        deposit_amount: U128,
    ) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        assert!(
            !next_validator_set
                .delegators
                .contains_key(&(delegator_id.clone(), validator_id.clone())),
            "The account {} is already been registered to validator {}.",
            &delegator_id,
            &validator_id
        );
        assert!(
            !self.unbonded_stakes.contains_key(&delegator_id),
            "The account {} is holding unbonded stake(s) which need to be withdrawn first.",
            &delegator_id
        );
        let protocol_settings = self.protocol_settings.get().unwrap();
        if let Some(v_ids) = next_validator_set
            .delegator_id_to_validator_ids
            .get(&delegator_id)
        {
            assert!(
                v_ids.len() < protocol_settings.maximum_validators_per_delegator.0,
                "Too many validators delegated."
            );
        }
        assert!(
            deposit_amount.0 >= protocol_settings.minimum_delegator_deposit.0,
            "The deposit for registering delegator is too few."
        );
        self.record_staking_fact(
            StakingFact::DelegatorRegistered {
                delegator_id,
                validator_id,
                amount: U128::from(deposit_amount),
            },
            &mut next_validator_set,
        );
    }
    //
    fn record_staking_fact(
        &mut self,
        staking_fact: StakingFact,
        next_validator_set: &mut ValidatorSet,
    ) -> u64 {
        let mut staking_histories = self.staking_histories.get().unwrap();
        let staking_history = staking_histories.append(staking_fact);
        self.staking_histories.set(&staking_histories);
        next_validator_set.apply_staking_history(&staking_history);
        self.next_validator_set.set(next_validator_set);
        staking_history.index.0
    }
    //
    fn increase_delegation(
        &mut self,
        delegator_id: AccountId,
        validator_id: AccountId,
        amount: U128,
    ) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        self.assert_delegator_id(&delegator_id, &validator_id, &next_validator_set);
        self.record_staking_fact(
            StakingFact::DelegationIncreased {
                delegator_id,
                validator_id,
                amount,
            },
            &mut next_validator_set,
        );
    }
}

#[near_bindgen]
impl StakingManager for AppchainAnchor {
    //
    fn decrease_stake(&mut self, amount: U128) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        let validator_id = env::predecessor_account_id();
        self.assert_validator_id(&validator_id, &next_validator_set);
        let protocol_settings = self.protocol_settings.get().unwrap();
        assert!(
            next_validator_set
                .validators
                .get(&validator_id)
                .unwrap()
                .deposit_amount
                - amount.0
                >= protocol_settings.minimum_validator_deposit.0,
            "Unable to decrease so much stake."
        );
        let index = self.record_staking_fact(
            StakingFact::StakeDecreased {
                validator_id: validator_id.clone(),
                amount,
            },
            &mut next_validator_set,
        );
        let mut unbond_stakes = match self.unbonded_stakes.contains_key(&validator_id) {
            true => self.unbonded_stakes.get(&validator_id).unwrap(),
            false => Vec::<UnbondedStakeReference>::new(),
        };
        unbond_stakes.push(UnbondedStakeReference {
            era_number: self
                .validator_set_histories
                .get()
                .unwrap()
                .index_range()
                .end_index
                .0
                + 1,
            staking_history_index: index,
        });
    }
    //
    fn unbond_stake(&mut self) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        let validator_id = env::predecessor_account_id();
        self.assert_validator_id(&validator_id, &next_validator_set);
        let validator = next_validator_set.validators.get(&validator_id).unwrap();
        let index = self.record_staking_fact(
            StakingFact::ValidatorUnbonded {
                validator_id: validator_id.clone(),
                amount: U128::from(validator.deposit_amount),
            },
            &mut next_validator_set,
        );
        let mut unbond_stakes = match self.unbonded_stakes.contains_key(&validator_id) {
            true => self.unbonded_stakes.get(&validator_id).unwrap(),
            false => Vec::<UnbondedStakeReference>::new(),
        };
        unbond_stakes.push(UnbondedStakeReference {
            era_number: self
                .validator_set_histories
                .get()
                .unwrap()
                .index_range()
                .end_index
                .0
                + 1,
            staking_history_index: index,
        });
    }
    //
    fn enable_delegation(&mut self) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        let validator_id = env::predecessor_account_id();
        self.assert_validator_id(&validator_id, &next_validator_set);
        self.record_staking_fact(
            StakingFact::ValidatorDelegationEnabled { validator_id },
            &mut next_validator_set,
        );
    }
    //
    fn disable_delegation(&mut self) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        let validator_id = env::predecessor_account_id();
        self.assert_validator_id(&validator_id, &next_validator_set);
        self.record_staking_fact(
            StakingFact::ValidatorDelegationDisabled { validator_id },
            &mut next_validator_set,
        );
    }
    //
    fn decrease_delegation(&mut self, validator_id: AccountId, amount: U128) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        let delegator_id = env::predecessor_account_id();
        self.assert_delegator_id(&delegator_id, &validator_id, &next_validator_set);
        let protocol_settings = self.protocol_settings.get().unwrap();
        assert!(
            next_validator_set
                .delegators
                .get(&(delegator_id.clone(), validator_id.clone()))
                .unwrap()
                .deposit_amount
                - amount.0
                >= protocol_settings.minimum_delegator_deposit.0,
            "Unable to decrease so much stake."
        );
        let index = self.record_staking_fact(
            StakingFact::DelegationDecreased {
                delegator_id: delegator_id.clone(),
                validator_id: validator_id.clone(),
                amount,
            },
            &mut next_validator_set,
        );
        let mut unbond_stakes = match self.unbonded_stakes.contains_key(&delegator_id) {
            true => self.unbonded_stakes.get(&delegator_id).unwrap(),
            false => Vec::<UnbondedStakeReference>::new(),
        };
        unbond_stakes.push(UnbondedStakeReference {
            era_number: self
                .validator_set_histories
                .get()
                .unwrap()
                .index_range()
                .end_index
                .0
                + 1,
            staking_history_index: index,
        });
    }
    //
    fn unbond_delegation(&mut self, validator_id: AccountId) {
        let mut next_validator_set = self.next_validator_set.get().unwrap();
        let delegator_id = env::predecessor_account_id();
        self.assert_delegator_id(&delegator_id, &validator_id, &next_validator_set);
        let delegator = next_validator_set
            .delegators
            .get(&(delegator_id.clone(), validator_id.clone()))
            .unwrap();
        let index = self.record_staking_fact(
            StakingFact::DelegatorUnbonded {
                delegator_id: delegator_id.clone(),
                validator_id: validator_id.clone(),
                amount: U128::from(delegator.deposit_amount),
            },
            &mut next_validator_set,
        );
        let mut unbond_stakes = match self.unbonded_stakes.contains_key(&delegator_id) {
            true => self.unbonded_stakes.get(&delegator_id).unwrap(),
            false => Vec::<UnbondedStakeReference>::new(),
        };
        unbond_stakes.push(UnbondedStakeReference {
            era_number: self
                .validator_set_histories
                .get()
                .unwrap()
                .index_range()
                .end_index
                .0
                + 1,
            staking_history_index: index,
        });
    }
    //
    fn withdraw_stake(&mut self, account_id: AccountId) {
        let protocol_settings = self.protocol_settings.get().unwrap();
        let mut balance_to_withdraw: u128 = 0;
        let mut remained_stakes = Vec::<UnbondedStakeReference>::new();
        if let Some(unbonded_stake_references) = self.unbonded_stakes.get(&account_id) {
            unbonded_stake_references.iter().for_each(|reference| {
                let validator_set = self
                    .validator_set_histories
                    .get()
                    .unwrap()
                    .get(&reference.era_number)
                    .unwrap();
                let staking_history = self
                    .staking_histories
                    .get()
                    .unwrap()
                    .get(&reference.staking_history_index)
                    .unwrap();
                match staking_history.staking_fact {
                    StakingFact::StakeDecreased {
                        validator_id: _,
                        amount,
                    }
                    | StakingFact::ValidatorUnbonded {
                        validator_id: _,
                        amount,
                    } => {
                        if validator_set.start_timestamp
                            + protocol_settings.unlock_period_of_validator_deposit.0
                                * SECONDS_OF_A_DAY
                                * NANO_SECONDS_MULTIPLE
                            > env::block_timestamp()
                        {
                            balance_to_withdraw += amount.0;
                        } else {
                            remained_stakes.push(reference.clone());
                        }
                    }
                    StakingFact::DelegationDecreased {
                        delegator_id: _,
                        validator_id: _,
                        amount,
                    }
                    | StakingFact::DelegatorUnbonded {
                        delegator_id: _,
                        validator_id: _,
                        amount,
                    } => {
                        if validator_set.start_timestamp
                            + protocol_settings.unlock_period_of_delegator_deposit.0
                                * SECONDS_OF_A_DAY
                                * NANO_SECONDS_MULTIPLE
                            > env::block_timestamp()
                        {
                            balance_to_withdraw += amount.0;
                        } else {
                            remained_stakes.push(reference.clone());
                        }
                    }
                    _ => (),
                };
            });
            if remained_stakes.len() > 0 {
                self.unbonded_stakes.insert(&account_id, &remained_stakes);
            } else {
                self.unbonded_stakes.remove(&account_id);
            }
            if balance_to_withdraw > 0 {
                ext_fungible_token::ft_transfer(
                    account_id,
                    balance_to_withdraw.into(),
                    None,
                    &self.oct_token.get().unwrap().contract_account,
                    1,
                    GAS_FOR_FT_TRANSFER_CALL,
                );
            }
        };
    }
    //
    fn withdraw_validator_rewards(&mut self, validator_id: AccountId) {
        let end_era = self
            .validator_set_histories
            .get()
            .unwrap()
            .index_range()
            .end_index
            .0;
        let protocol_settings = self.protocol_settings.get().unwrap();
        let start_era = end_era - protocol_settings.maximum_era_count_of_unwithdrawn_reward.0;
        let mut reward_to_withdraw: u128 = 0;
        for era_number in start_era..end_era {
            if let Some(reward) = self
                .unwithdrawn_validator_rewards
                .get(&(era_number, validator_id.clone()))
            {
                reward_to_withdraw += reward;
                self.unwithdrawn_validator_rewards
                    .remove(&(era_number, validator_id.clone()));
            }
        }
        if reward_to_withdraw > 0 {
            ext_fungible_token::ft_transfer(
                validator_id,
                reward_to_withdraw.into(),
                None,
                &self.wrapped_appchain_token.get().unwrap().contract_account,
                1,
                GAS_FOR_FT_TRANSFER_CALL,
            );
        }
    }
    //
    fn withdraw_delegator_rewards(&mut self, delegator_id: AccountId, validator_id: AccountId) {
        let end_era = self
            .validator_set_histories
            .get()
            .unwrap()
            .index_range()
            .end_index
            .0;
        let protocol_settings = self.protocol_settings.get().unwrap();
        let start_era = end_era - protocol_settings.maximum_era_count_of_unwithdrawn_reward.0;
        let mut reward_to_withdraw: u128 = 0;
        for era_number in start_era..end_era {
            if let Some(reward) = self.unwithdrawn_delegator_rewards.get(&(
                era_number,
                delegator_id.clone(),
                validator_id.clone(),
            )) {
                reward_to_withdraw += reward;
                self.unwithdrawn_delegator_rewards.remove(&(
                    era_number,
                    delegator_id.clone(),
                    validator_id.clone(),
                ));
            }
        }
        if reward_to_withdraw > 0 {
            ext_fungible_token::ft_transfer(
                delegator_id,
                reward_to_withdraw.into(),
                None,
                &self.wrapped_appchain_token.get().unwrap().contract_account,
                1,
                GAS_FOR_FT_TRANSFER_CALL,
            );
        }
    }
}
