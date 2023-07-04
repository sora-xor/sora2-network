use crate::*;

pub type Migrations = (
    vested_rewards::migrations::v4::Migration<Runtime>,
    xst::migrations::CustomSyntheticsUpgrade<Runtime>,
);
