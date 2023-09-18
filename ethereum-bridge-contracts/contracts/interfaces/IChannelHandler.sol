// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.15;
import "./IBatch.sol";

interface IChannelHandler is IBatch {

    event ChangePeers(address peerId, bool removal);

    function submit(
        Batch calldata batch,
        uint8[] calldata v,
        bytes32[] calldata r,
        bytes32[] calldata s
    ) external;
    function submitMessage(bytes calldata payload) external;
    function removePeerByPeer(address peerAddress) external returns (bool);
    function addPeerByPeer(address peerAddress) external returns (bool);
    function registerApp(address newApp) external returns (bool);
    function removeApp(address app) external returns (bool);

}
