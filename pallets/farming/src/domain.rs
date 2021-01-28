use super::*;
use common::balance::Balance;
use core::convert::TryInto;
use frame_support::fail;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::cmp::PartialOrd;
use sp_std::ops::{AddAssign, Mul, Sub, SubAssign};

pub type FarmId = u64;

type CanFail = Result<(), ()>;

macro_rules! guard {
    ( $predicate:expr ) => {
        debug_assert!($predicate);
        #[cfg(not(skip_guards))]
        {
            if !$predicate {
                fail!(());
            }
        }
    };
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct FarmingState<Balance, BlockNumber> {
    pub units_per_blocks: Balance,
    pub last_change: BlockNumber,
    pub units_locked: Balance,
}

impl<Balance, BlockNumber> FarmingState<Balance, BlockNumber>
where
    BlockNumber:
        Copy + TryInto<u32> + PartialOrd + Sub<Output = BlockNumber> + Mul<Output = BlockNumber>,
    Balance: Copy + From<u32> + Mul<Output = Balance> + AddAssign + PartialOrd + SubAssign,
{
    /// Recalculate state, update per blocks value and set last change to current point.
    pub fn recalculate(&mut self, current_block: BlockNumber) -> CanFail {
        guard!(self.last_change <= current_block);
        if self.last_change != current_block {
            //FIXME: For u32 type is always ok as far as i know, maybe it is ok.
            let n_blocks_u32: u32 = (current_block - self.last_change)
                .try_into()
                .ok()
                .expect("This calculation must success, this means is always at least 32-bit; qed");
            let n_blocks: Balance = n_blocks_u32.into();
            self.units_per_blocks += n_blocks * self.units_locked;
            self.last_change = current_block;
        }
        Ok(())
    }

    /// Put amount of liquidity units to locked value, this means to change field of "locked now"
    /// after recalculation, agg_state is option because can be used to modify aggregated state.
    pub fn put_to_locked(
        &mut self,
        agg_state: Option<&mut Self>,
        current_block: BlockNumber,
        value: Balance,
    ) -> CanFail {
        self.recalculate(current_block)?;
        self.units_locked += value;
        match agg_state {
            Some(ags) => ags.put_to_locked(None, current_block, value)?,
            _ => (),
        }
        Ok(())
    }

    /// Remove amount of liquididy units from "locked now" field after recalculation.
    /// Like in previous function agg_state means
    /// aggragated state and used to update aggregated values if needed.
    pub fn remove_from_locked(
        &mut self,
        agg_state: Option<&mut Self>,
        current_block: BlockNumber,
        value: Balance,
    ) -> CanFail {
        guard!(self.units_locked >= value);
        self.recalculate(current_block)?;
        self.units_locked -= value;
        match agg_state {
            Some(ags) => ags.remove_from_locked(None, current_block, value)?,
            _ => (),
        }
        Ok(())
    }

    /// Remove all liquidity units from "locked now". Same recalculation logic and agg_state
    /// meaning.
    pub fn remove_all_from_locked(
        &mut self,
        agg_state: Option<&mut Self>,
        current_block: BlockNumber,
    ) -> CanFail {
        self.recalculate(current_block)?;
        match agg_state {
            Some(ags) => ags.remove_from_locked(None, current_block, self.units_locked)?,
            _ => (),
        }
        self.units_locked = 0u32.into();
        Ok(())
    }

    /// Remove units from units per blocks field, used in claiming.
    /// Same recalculation logic and agg_state meaning.
    pub fn remove_from_upb(
        &mut self,
        agg_state: Option<&mut Self>,
        current_block: BlockNumber,
        value: Balance,
    ) -> CanFail {
        guard!(self.units_per_blocks >= value);
        self.recalculate(current_block)?;
        self.units_per_blocks -= value;
        match agg_state {
            Some(ags) => ags.remove_from_upb(None, current_block, value)?,
            _ => (),
        }
        Ok(())
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct IncentiveModel<AssetId, Balance, BlockNumber> {
    pub suitable_for_block: BlockNumber,
    // Origin asset is source of insentive, burned for example.
    pub origin_asset_id: AssetId,
    // Claim asset is asset that will be sended to farmer, for example it can be minded because
    // origin asset is burned and separated as allowed for this.
    pub claim_asset_id: AssetId,
    // Amount of origin asset that was separated for claim, for example subset of burned amount.
    pub amount_of_origin: Option<Balance>,
    // Price ratio used to virtually convert origin asset amount to claim asset amount,
    // for example claim is minted for origin burned and separated.
    pub origin_to_claim_ratio: Option<Balance>,
}

/// Farm container with parameters and information about tokens and owner.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Farm<AccountId, AssetId, BlockNumber> {
    pub id: FarmId,
    pub owner_id: AccountId,
    pub creation_block_number: BlockNumber,
    // This state is aggregated states of farmers, that can be updated incrementally.
    pub aggregated_state: FarmingState<Balance, BlockNumber>,
    // This is state of incentive model, that will be updated every time
    pub incentive_model_state: IncentiveModel<AssetId, Balance, BlockNumber>,
}

/// Farmer representation.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Farmer<AccountId, TechAccountId, BlockNumber> {
    pub id: (FarmId, AccountId),
    pub tech_account_id: TechAccountId,
    pub state: FarmingState<Balance, BlockNumber>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct FarmInfo<AccountId, AssetId, BlockNumber> {
    pub farm: Farm<AccountId, AssetId, BlockNumber>,
    pub total_upbu_now: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct FarmerInfo<AccountId, TechAccountId, BlockNumber> {
    pub farmer: Farmer<AccountId, TechAccountId, BlockNumber>,
    pub upbu_now: Balance,
}
