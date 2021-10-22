use near_sdk::BlockHeight;

use crate::*;

/// Appchain validator of an appchain.
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Validator {
    /// The validator's id in NEAR protocol.
    pub validator_id: AccountId,
    /// The validator's id in the appchain.
    pub validator_id_in_appchain: String,
    /// The block height when the validator is registered.
    pub registered_block_height: BlockHeight,
    /// The timestamp when the validator is registered.
    pub registered_timestamp: Timestamp,
    /// Total deposited balance of the validator.
    pub deposit_amount: Balance,
    /// Total stake of the validator, including delegations of all delegators.
    pub total_stake: Balance,
    /// Whether the validator accepts delegation from delegators.
    pub can_be_delegated_to: bool,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Delegator {
    /// The delegator's id in NEAR protocol.
    pub delegator_id: AccountId,
    /// The validator's id in NEAR protocol, which the delegator delegates his rights to.
    pub validator_id: AccountId,
    /// The block height when the delegator is registered.
    pub registered_block_height: BlockHeight,
    /// The timestamp when the delegator is registered.
    pub registered_timestamp: Timestamp,
    /// Delegated balance of the delegator.
    pub deposit_amount: Balance,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ValidatorSet {
    /// The number of era in appchain.
    pub era_number: u64,
    /// The set of account id of validators.
    pub validator_id_set: UnorderedSet<AccountId>,
    /// The map from validator id to the set of its delegators' id.
    pub validator_id_to_delegator_id_set: LookupMap<AccountId, UnorderedSet<AccountId>>,
    /// The map from delegator id to the set of its validators' id that
    /// the delegator delegates his/her voting rights to.
    pub delegator_id_to_validator_id_set: LookupMap<AccountId, UnorderedSet<AccountId>>,
    /// The validators data, mapped by their account id in NEAR protocol.
    pub validators: LookupMap<AccountId, Validator>,
    /// The delegators data, mapped by the tuple of their delegator account id and
    /// validator account id in NEAR protocol.
    pub delegators: LookupMap<(AccountId, AccountId), Delegator>,
    /// Total stake of current set
    pub total_stake: Balance,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ValidatorSetOfEra {
    /// The validator set of this era
    pub validator_set: ValidatorSet,
    /// The validator list for query
    pub validator_list: Vector<AppchainValidator>,
    /// The block height when the era starts.
    pub start_block_height: BlockHeight,
    /// The timestamp when the era starts.
    pub start_timestamp: Timestamp,
    /// The index of the latest staking history happened in the era of corresponding appchain.
    pub staking_history_index: u64,
    /// The set of validator id which will not be profited.
    pub unprofitable_validator_id_set: UnorderedSet<AccountId>,
    /// Total stake excluding all unprofitable validators' stake.
    pub valid_total_stake: Balance,
    /// The rewards of validators in this era
    pub validator_rewards: LookupMap<AccountId, Balance>,
    /// The rewards of delegators in this era
    pub delegator_rewards: LookupMap<(AccountId, AccountId), Balance>,
    /// The status of creation of this set
    pub processing_status: ValidatorSetProcessingStatus,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct ValidatorSetHistories {
    /// The history version of validator set, mapped by era number in appchain.
    histories: LookupMap<u64, ValidatorSetOfEra>,
    /// The start index of valid validator set.
    start_index: u64,
    /// The end index of valid validator set.
    end_index: u64,
}

impl ValidatorSetHistories {
    ///
    pub fn new() -> Self {
        Self {
            histories: LookupMap::new(StorageKey::ValidatorSetHistoriesMap.into_bytes()),
            start_index: 0,
            end_index: 0,
        }
    }
    ///
    pub fn index_range(&self) -> IndexRange {
        IndexRange {
            start_index: U64::from(self.start_index),
            end_index: U64::from(self.end_index),
        }
    }
    ///
    pub fn contains(&self, era_number: &u64) -> bool {
        self.histories.contains_key(era_number)
    }
    ///
    pub fn get(&self, index: &u64) -> Option<ValidatorSetOfEra> {
        self.histories.get(index)
    }
    ///
    pub fn insert(&mut self, era_number: &u64, validator_set: &ValidatorSetOfEra) {
        self.histories.insert(era_number, validator_set);
        if *era_number > self.end_index {
            self.end_index = *era_number;
        }
    }
}

pub trait ValidatorSetActions {
    /// Apply a certain `staking history` to the validator set.
    fn apply_staking_history(&mut self, staking_history: &StakingHistory);
}

impl ValidatorSet {
    ///
    pub fn new(era_number: u64) -> Self {
        Self {
            era_number,
            validator_id_set: UnorderedSet::new(
                StorageKey::ValidatorIdsOfEra(era_number).into_bytes(),
            ),
            validator_id_to_delegator_id_set: LookupMap::new(
                StorageKey::ValidatorToDelegatorsMapOfEra(era_number).into_bytes(),
            ),
            delegator_id_to_validator_id_set: LookupMap::new(
                StorageKey::DelegatorToValidatorsMapOfEra(era_number).into_bytes(),
            ),
            validators: LookupMap::new(StorageKey::ValidatorsOfEra(era_number).into_bytes()),
            delegators: LookupMap::new(StorageKey::DelegatorsOfEra(era_number).into_bytes()),
            total_stake: 0,
        }
    }
}

impl ValidatorSetActions for ValidatorSet {
    //
    fn apply_staking_history(&mut self, staking_history: &StakingHistory) {
        match &staking_history.staking_fact {
            types::StakingFact::ValidatorRegistered {
                validator_id,
                validator_id_in_appchain,
                amount,
                can_be_delegated_to,
            } => {
                self.validator_id_set.insert(validator_id);
                self.validators.insert(
                    validator_id,
                    &Validator {
                        validator_id: validator_id.clone(),
                        validator_id_in_appchain: validator_id_in_appchain.to_string(),
                        registered_block_height: env::block_index(),
                        registered_timestamp: env::block_timestamp(),
                        deposit_amount: amount.0,
                        total_stake: amount.0,
                        can_be_delegated_to: *can_be_delegated_to,
                    },
                );
                self.total_stake += amount.0;
            }
            types::StakingFact::StakeIncreased {
                validator_id,
                amount,
            } => {
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.deposit_amount += amount.0;
                validator.total_stake += amount.0;
                self.validators.insert(validator_id, &validator);
                self.total_stake += amount.0;
            }
            types::StakingFact::StakeDecreased {
                validator_id,
                amount,
            } => {
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.deposit_amount -= amount.0;
                validator.total_stake -= amount.0;
                self.validators.insert(validator_id, &validator);
                self.total_stake -= amount.0;
            }
            types::StakingFact::ValidatorUnbonded {
                validator_id,
                amount: _,
            } => {
                if let Some(delegator_id_set) =
                    self.validator_id_to_delegator_id_set.get(validator_id)
                {
                    let delegator_ids = delegator_id_set.to_vec();
                    delegator_ids.iter().for_each(|delegator_id| {
                        self.delegators
                            .remove(&(delegator_id.clone(), validator_id.clone()));
                        if let Some(mut validator_id_set) =
                            self.delegator_id_to_validator_id_set.get(delegator_id)
                        {
                            validator_id_set.remove(validator_id);
                            if validator_id_set.len() > 0 {
                                self.delegator_id_to_validator_id_set
                                    .insert(delegator_id, &validator_id_set);
                            } else {
                                self.delegator_id_to_validator_id_set.remove(delegator_id);
                            }
                        }
                    });
                    self.validator_id_to_delegator_id_set.remove(validator_id);
                }
                let validator = self.validators.remove(validator_id).unwrap();
                self.total_stake -= validator.total_stake;
                self.validator_id_set.remove(validator_id);
            }
            types::StakingFact::ValidatorDelegationEnabled { validator_id } => {
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.can_be_delegated_to = true;
                self.validators.insert(validator_id, &validator);
            }
            types::StakingFact::ValidatorDelegationDisabled { validator_id } => {
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.can_be_delegated_to = false;
                self.validators.insert(validator_id, &validator);
            }
            types::StakingFact::DelegatorRegistered {
                delegator_id,
                validator_id,
                amount,
            } => {
                self.delegators.insert(
                    &(delegator_id.clone(), validator_id.clone()),
                    &Delegator {
                        delegator_id: delegator_id.clone(),
                        validator_id: validator_id.clone(),
                        registered_block_height: env::block_index(),
                        registered_timestamp: env::block_timestamp(),
                        deposit_amount: amount.0,
                    },
                );
                if !self
                    .validator_id_to_delegator_id_set
                    .contains_key(validator_id)
                {
                    self.validator_id_to_delegator_id_set.insert(
                        validator_id,
                        &UnorderedSet::new(
                            StorageKey::DelegatorIdsInMapOfVToDOfEra {
                                era_number: self.era_number,
                                validator_id: validator_id.clone(),
                            }
                            .into_bytes(),
                        ),
                    );
                }
                let mut delegator_id_set = self
                    .validator_id_to_delegator_id_set
                    .get(validator_id)
                    .unwrap();
                delegator_id_set.insert(delegator_id);
                self.validator_id_to_delegator_id_set
                    .insert(validator_id, &delegator_id_set);
                if !self
                    .delegator_id_to_validator_id_set
                    .contains_key(delegator_id)
                {
                    self.delegator_id_to_validator_id_set.insert(
                        delegator_id,
                        &UnorderedSet::new(
                            StorageKey::ValidatorIdsInMapOfDToVOfEra {
                                era_number: self.era_number,
                                delegator_id: delegator_id.clone(),
                            }
                            .into_bytes(),
                        ),
                    );
                }
                let mut validator_id_set = self
                    .delegator_id_to_validator_id_set
                    .get(delegator_id)
                    .unwrap();
                validator_id_set.insert(validator_id);
                self.delegator_id_to_validator_id_set
                    .insert(delegator_id, &validator_id_set);
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.total_stake += amount.0;
                self.validators.insert(validator_id, &validator);
                self.total_stake += amount.0;
            }
            types::StakingFact::DelegationIncreased {
                delegator_id,
                validator_id,
                amount,
            } => {
                let mut delegator = self
                    .delegators
                    .get(&(delegator_id.clone(), validator_id.clone()))
                    .unwrap();
                delegator.deposit_amount += amount.0;
                self.delegators
                    .insert(&(delegator_id.clone(), validator_id.clone()), &delegator);
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.total_stake += amount.0;
                self.validators.insert(validator_id, &validator);
                self.total_stake += amount.0;
            }
            types::StakingFact::DelegationDecreased {
                delegator_id,
                validator_id,
                amount,
            } => {
                let mut delegator = self
                    .delegators
                    .get(&(delegator_id.clone(), validator_id.clone()))
                    .unwrap();
                delegator.deposit_amount -= amount.0;
                self.delegators
                    .insert(&(delegator_id.clone(), validator_id.clone()), &delegator);
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.total_stake -= amount.0;
                self.validators.insert(validator_id, &validator);
                self.total_stake -= amount.0;
            }
            types::StakingFact::DelegatorUnbonded {
                delegator_id,
                validator_id,
                amount: _,
            } => {
                let mut delegator_id_set = self
                    .validator_id_to_delegator_id_set
                    .get(validator_id)
                    .unwrap();
                delegator_id_set.remove(delegator_id);
                if delegator_id_set.len() > 0 {
                    self.validator_id_to_delegator_id_set
                        .insert(validator_id, &delegator_id_set);
                } else {
                    self.validator_id_to_delegator_id_set.remove(validator_id);
                }
                let mut validator_id_set = self
                    .delegator_id_to_validator_id_set
                    .get(delegator_id)
                    .unwrap();
                validator_id_set.remove(validator_id);
                if validator_id_set.len() > 0 {
                    self.delegator_id_to_validator_id_set
                        .insert(delegator_id, &validator_id_set);
                } else {
                    self.delegator_id_to_validator_id_set.remove(delegator_id);
                }
                let delegator = self
                    .delegators
                    .remove(&(delegator_id.clone(), validator_id.clone()))
                    .unwrap();
                let mut validator = self.validators.get(validator_id).unwrap();
                validator.total_stake -= delegator.deposit_amount;
                self.validators.insert(validator_id, &validator);
                self.total_stake -= delegator.deposit_amount;
            }
        }
    }
}

impl ValidatorSetProcessingStatus {
    ///
    pub fn is_ready_for_distributing_reward(&self) -> bool {
        match self {
            ValidatorSetProcessingStatus::CopyingFromLastEra {
                copying_validator_index: _,
                copying_delegator_index: _,
            } => false,
            ValidatorSetProcessingStatus::ApplyingStakingHistory { applying_index: _ } => false,
            ValidatorSetProcessingStatus::MakingValidatorList { making_index: _ } => false,
            ValidatorSetProcessingStatus::ReadyForDistributingReward => true,
            ValidatorSetProcessingStatus::DistributingReward {
                distributing_validator_index: _,
                distributing_delegator_index: _,
            } => false,
            ValidatorSetProcessingStatus::Completed => false,
        }
    }
}

impl ValidatorSetOfEra {
    ///
    pub fn new(era_number: u64, staking_history_index: u64) -> Self {
        Self {
            start_block_height: env::block_index(),
            start_timestamp: env::block_timestamp(),
            staking_history_index,
            unprofitable_validator_id_set: UnorderedSet::new(
                StorageKey::UnprofitableValidatorIdsOfEra(era_number).into_bytes(),
            ),
            validator_set: ValidatorSet::new(era_number),
            validator_list: Vector::new(StorageKey::ValidatorListOfEra(era_number).into_bytes()),
            valid_total_stake: 0,
            validator_rewards: LookupMap::new(
                StorageKey::ValidatorRewardsOfEra(era_number).into_bytes(),
            ),
            delegator_rewards: LookupMap::new(
                StorageKey::DelegatorRewardsOfEra(era_number).into_bytes(),
            ),
            processing_status: ValidatorSetProcessingStatus::CopyingFromLastEra {
                copying_validator_index: U64::from(0),
                copying_delegator_index: U64::from(0),
            },
        }
    }
    ///
    pub fn set_unprofitable_validator_ids(&mut self, unprofitable_validator_ids: Vec<AccountId>) {
        unprofitable_validator_ids.iter().for_each(|v_id| {
            self.unprofitable_validator_id_set.insert(&v_id);
        });
    }
    ///
    pub fn calculate_valid_total_stake(&mut self) {
        let unprofitable_validator_ids = self.unprofitable_validator_id_set.to_vec();
        self.valid_total_stake = self.validator_set.total_stake;
        unprofitable_validator_ids.iter().for_each(|v_id| {
            let validator = self.validator_set.validators.get(v_id).unwrap();
            self.valid_total_stake -= validator.total_stake;
        });
    }
    ///
    pub fn to_validator_set_info(&self) -> ValidatorSetInfo {
        ValidatorSetInfo {
            era_number: U64::from(self.validator_set.era_number),
            total_stake: U128::from(self.validator_set.total_stake),
            validator_list: self.validator_list.to_vec(),
            start_block_height: U64::from(self.start_block_height),
            start_timestamp: U64::from(self.start_timestamp),
            staking_history_index: U64::from(self.staking_history_index),
            unprofitable_validator_ids: self.unprofitable_validator_id_set.to_vec(),
            valid_total_stake: U128::from(self.valid_total_stake),
            processing_status: self.processing_status.clone(),
        }
    }
}

impl ValidatorSetActions for ValidatorSetOfEra {
    //
    fn apply_staking_history(&mut self, staking_history: &StakingHistory) {
        self.validator_set.apply_staking_history(staking_history);
    }
}
