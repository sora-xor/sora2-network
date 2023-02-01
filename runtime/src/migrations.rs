use crate::*;
use frame_support::traits::OnRuntimeUpgrade;

pub type Migrations =
    (multicollateral_bonding_curve_pool::migrations::v2::InitializeTBCD<Runtime>,);
