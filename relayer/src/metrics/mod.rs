pub mod ethereum;
pub mod para;
pub mod sora;

pub const ETH_TOTAL_RPC_REQUESTS: &str = "bridge_eth_total_rpc_requests";
pub const SUB_TOTAL_RPC_REQUESTS: &str = "bridge_sub_total_rpc_requests";

pub fn describe_metrics() {
    metrics::describe_counter!(
        ETH_TOTAL_RPC_REQUESTS,
        "Total RPC requests sent by Ethereum client"
    );
    metrics::describe_counter!(
        SUB_TOTAL_RPC_REQUESTS,
        "Total RPC requests sent by Substrate client"
    );

    ethereum::describe_metrics();
    sora::describe_metrics();
    para::describe_metrics();
}
