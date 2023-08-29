import { ethers } from 'hardhat';
import { expect } from 'chai';
import { SignerWithAddress } from '@nomicfoundation/hardhat-ethers/signers';
import { InboundChannel } from '../typechain/contracts/InboundChannel';
import { OutboundChannel } from '../typechain/contracts/OutboundChannel';
import { InboundChannel__factory } from '../typechain/factories/contracts/InboundChannel__factory';
import { OutboundChannel__factory } from '../typechain/factories/contracts/OutboundChannel__factory';

describe("Inbound channel", function () {
  let peers: SignerWithAddress[]
  let inboundChannel: InboundChannel;
  let inboundFactory: InboundChannel__factory;
  let outboundFactory: OutboundChannel__factory;
  let outboundChannel: OutboundChannel;
  const coder = ethers.AbiCoder.defaultAbiCoder()
  const hhChainIdReversed = "47708404443145933667408199115060285764373720183435463107199610647303319191552";

  before(async function () {
    peers = await ethers.getSigners();   
    inboundFactory = await ethers.getContractFactory('InboundChannel');
    outboundFactory = await ethers.getContractFactory('OutboundChannel');
    inboundChannel = await (await inboundFactory.deploy()).waitForDeployment()
    outboundChannel = await (await outboundFactory.deploy()).waitForDeployment()
    await inboundChannel.initialize(await outboundChannel.getAddress(), [peers[0].address])
  });

  it("should_revert_on_invalid_nonce", async function () {
    const batch = {
      nonce: 3,
      total_max_gas: 6,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await expect(inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).to.be.revertedWith("invalid batch nonce")
  });

  it("should_revert_on_invalid_nonce_due_to_duplication", async function () {
    const batch = {
      nonce: 1,
      total_max_gas: 6,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s]);
    await expect(inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).to.be.revertedWith("invalid batch nonce");
  });

  it("should_revert_on_invalid_signature_nonce_mismatch", async function () {
    let batch = {
      nonce: 2,
      total_max_gas: 6,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s]);
    batch.nonce = 3;
    await expect(inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).to.be.revertedWith("Invalid signatures");
  });

  it("should_revert_on_invalid_signature", async function () {
    const batch = {
      nonce: 3,
      total_max_gas: 6,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await expect(inboundChannel.submit(batch, [signature.v + 3], [signature.r], [signature.s])).to.be.revertedWith("ECDSA: invalid signature 'v' value");
  });

  it("should_revert_on_invalid_signature_v_length_mismatch", async function () {
    const batch = {
      nonce: 3,
      total_max_gas: 6,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await expect(inboundChannel.submit(batch, [signature.v, signature.v], [signature.r], [signature.s])).to.be.revertedWith("v and r length mismatch");
  });

  it("should_revert_on_invalid_signature_s_length_mismatch", async function () {
    const batch = {
      nonce: 3,
      total_max_gas: 6,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await expect(inboundChannel.submit(batch, [signature.v, signature.v], [signature.r, signature.r], [signature.s])).to.be.revertedWith("v and s length mismatch");
  });

  it("should_submit_signed_random_data", async function () {
    const batch = {
      nonce: 3,
      total_max_gas: 6,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])
  });

  it("should_add_a_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("addPeerByPeer", [peers[1].address])
    const batch = {
      nonce: 4,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(1);
    await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    expect(await inboundChannel.isPeer(peers[1].address)).to.be.equal(true);
  });

  it("should_add_another_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("addPeerByPeer", [peers[2].address])
    const batch = {
      nonce: 5,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let signature2 = ethers.Signature.from(await peers[1].signMessage(ethers.getBytes(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    await inboundChannel.submit(batch, [signature.v, signature2.v], [signature.r, signature2.r], [signature.s, signature2.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(3);
    expect(await inboundChannel.isPeer(peers[2].address)).to.be.equal(true);
  });

  it("should_remove_a_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[2].address])
    const batch = {
      nonce: 6,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let signature2 = ethers.Signature.from(await peers[1].signMessage(ethers.getBytes(encodedMessage)));
    let signature3 = ethers.Signature.from(await peers[2].signMessage(ethers.getBytes(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(3);
    await inboundChannel.submit(batch, [signature.v, signature2.v, signature3.v], [signature.r, signature2.r, signature3.r], [signature.s, signature2.s, signature3.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    expect(await inboundChannel.isPeer(peers[2].address)).to.be.equal(false);
  });

  it("should_not_remove_a_peer(if already removed)", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[2].address])
    const batch = {
      nonce: 7,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let signature2 = ethers.Signature.from(await peers[1].signMessage(ethers.getBytes(encodedMessage)));
    let tx = await(await inboundChannel.submit(batch, [signature.v, signature2.v], [signature.r, signature2.r], [signature.s, signature2.s])).wait();
    expect(await inboundChannel.peersCount()).to.be.equal(2);
  });

  it("should_not_add_a_peer(if already added)", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("addPeerByPeer", [peers[1].address])
    const batch = {
      nonce: 8,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let signature2 = ethers.Signature.from(await peers[1].signMessage(ethers.getBytes(encodedMessage)));
    let tx = await(await inboundChannel.submit(batch, [signature.v, signature2.v], [signature.r, signature2.r], [signature.s, signature2.s])).wait();
    let decoder = inboundChannel.interface.decodeEventLog("BatchDispatched(uint256, address, uint256, uint256, uint256, uint256)", tx.logs[0].data);
    expect(decoder[2]).to.be.equal(0); // false result 
  });

  it("should_revert_on_add_a_peer", async function () {
    await expect(inboundChannel.addPeerByPeer(peers[2].address)).to.be.revertedWith('caller not this contract');
  });

  it("should_revert_on_remove_a_peer", async function () {
    await expect(inboundChannel.removePeerByPeer(peers[1].address)).to.be.revertedWith('caller not this contract');
  });

  it("should_revert_to_remove_a_peer(not enough sigs)", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[1].address])
    const batch = {
      nonce: 9,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await expect(inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).to.be.revertedWith('not enough signatures');
  });

  it("should_remove_another_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[1].address])
    const batch = {
      nonce: 9,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let signature2 = ethers.Signature.from(await peers[1].signMessage(ethers.getBytes(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    await inboundChannel.submit(batch, [signature.v, signature2.v], [signature.r, signature2.r], [signature.s, signature2.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(1);
    expect(await inboundChannel.isPeer(peers[1].address)).to.be.equal(false);
  });

  it("should_not_remove_last_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[0].address])
    const batch = {
      nonce: 10,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(1);
    await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(1);
    expect(await inboundChannel.isPeer(peers[0].address)).to.be.equal(true);
  });

  it("should_revert_on_submit_signed_random_data(batch gas)", async function () {
    const batch = {
      nonce: 11,
      total_max_gas: 30000000,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await expect(inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).to.be.rejectedWith('insufficient gas for delivery of all messages')
  });

  it("should_revert_on_submit_signed_random_data(msg lenght)", async function () {
    let batch = {
      nonce: 11,
      total_max_gas: 1000000,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }
    for (let i = 0; i < 256; i++) {
      batch.messages.push(batch.messages[0])
    }
    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    await expect(inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).to.be.rejectedWith('must be < 256 messages in the batch')
  });

  it("should_submit_signed_random_data(huge chunk)", async function () {
    let batch = {
      nonce: 11,
      total_max_gas: 1000000,
      messages: [{
        target: peers[0].address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: peers[1].address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }
    for (let i = 0; i < 250; i++) {
      batch.messages.push(batch.messages[0])
    }
    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let tx = await(await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).wait();
    let decoder = inboundChannel.interface.decodeEventLog("BatchDispatched(uint256, address, uint256, uint256, uint256, uint256)", tx.logs[0].data)
    console.log("dispatch result:", decoder[2]);
    console.log("gas spent:", decoder[4]);
  });

  it("should_submit_signed_random_data(huge chunk all failed)", async function () {
    let batch = {
      nonce: 12,
      total_max_gas: 1000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }
    for (let i = 0; i < 250; i++) {
      batch.messages.push(batch.messages[0])
    }
    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    console.log("encodedMessage:", encodedMessage);
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let tx = await (await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).wait();
    let decoder = inboundChannel.interface.decodeEventLog("BatchDispatched(uint256, address, uint256, uint256, uint256, uint256)", tx.logs[0].data)
    expect(decoder[2]).to.be.equal(0); // false result 
    console.log("gas spent:", decoder[4]);
  });

  it("submit_signed_random_data(Transaction gas limit does not exceed block gas limit of 30000000 after update)", async function () {
    let batch = {
      nonce: 13,
      total_max_gas: 20000000,
      messages: [{
        target: await inboundChannel.getAddress(),
        max_gas: 1,
        payload: "0xe07bc27e9b5ec4da29ece7c092db9c1d93331db1e3836d7d3c2a8e4efdd45126e07bc27e9b5ec4da29ece7c092db9c1d93331db1e3836d7d3c2a8e4efdd45126",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 2,
        payload: "0xe07bc27e9b5ec4da29ece7c092db9c1d93331db1e3836d7d3c2a8e4efdd45126",
      },
      {
        target: await inboundChannel.getAddress(),
        max_gas: 3,
        payload: "0xe07bc27e9b5ec4da29ece7c092db9c1d93331db1e3836d7d3c2a8e4efdd45126",
      }]
    }
    for (let i = 0; i < 250; i++) {
      batch.messages.push(batch.messages[0])
    }
    let commitment = ethers.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]));
    let encodedMessage = ethers.keccak256(coder.encode(["uint",  "bytes32"], [hhChainIdReversed, commitment]));
    console.log("encodedMessage:", encodedMessage);
    let signature = ethers.Signature.from(await peers[0].signMessage(ethers.getBytes(encodedMessage)));
    let tx = await(await inboundChannel.submit(batch, [signature.v], [signature.r], [signature.s])).wait();
    let decoder = inboundChannel.interface.decodeEventLog("BatchDispatched(uint256, address, uint256, uint256, uint256, uint256)", tx.logs[0].data);
    expect(decoder[2]).to.be.equal(0); // false result 
    console.log("gas spent:", decoder[4]);
    console.log("gas used:", tx.gasUsed);
  });

});
