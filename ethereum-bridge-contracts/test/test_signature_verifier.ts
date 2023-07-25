import { ethers } from 'hardhat';
import { expect } from 'chai';
import { SignerWithAddress } from '@nomiclabs/hardhat-ethers/signers';
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
  const coder = ethers.utils.defaultAbiCoder

  before(async function () {
    peers = await ethers.getSigners();
    inboundFactory = (await ethers.getContractFactory('InboundChannel')) as InboundChannel__factory;
    outboundFactory = (await ethers.getContractFactory('OutboundChannel')) as OutboundChannel__factory;
    inboundChannel = await (await inboundFactory.deploy()).deployed()
    outboundChannel = await (await outboundFactory.deploy()).deployed()
    await inboundChannel.initialize(outboundChannel.address, [peers[0].address])
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.revertedWith("invalid batch nonce")
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s]);
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.revertedWith("invalid batch nonce");
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s]);
    batch.nonce = 3;
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.revertedWith("Invalid signatures");
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v).add(3)], [signature.r], [signature.s])).to.be.revertedWith("ECDSA: invalid signature 'v' value");
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.revertedWith("v and r length mismatch");
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature.v)], [signature.r, signature.r], [signature.s])).to.be.revertedWith("v and s length mismatch");
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])
  });

  it("should_add_a_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("addPeerByPeer", [peers[1].address])
    const batch = {
      nonce: 4,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(1);
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    expect(await inboundChannel.isPeer(peers[1].address)).to.be.equal(true);
  });

  it("should_add_another_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("addPeerByPeer", [peers[2].address])
    const batch = {
      nonce: 5,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    let signature2 = ethers.utils.splitSignature(await peers[1].signMessage(ethers.utils.arrayify(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature2.v)], [signature.r, signature2.r], [signature.s, signature2.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(3);
    expect(await inboundChannel.isPeer(peers[2].address)).to.be.equal(true);
  });

  it("should_remove_a_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[2].address])
    const batch = {
      nonce: 6,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    let signature2 = ethers.utils.splitSignature(await peers[1].signMessage(ethers.utils.arrayify(encodedMessage)));
    let signature3 = ethers.utils.splitSignature(await peers[2].signMessage(ethers.utils.arrayify(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(3);
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature2.v), ethers.BigNumber.from(signature3.v)], [signature.r, signature2.r, signature3.r], [signature.s, signature2.s, signature3.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    expect(await inboundChannel.isPeer(peers[2].address)).to.be.equal(false);
  });

  it("should_not_remove_a_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[2].address])
    const batch = {
      nonce: 7,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    let signature2 = ethers.utils.splitSignature(await peers[1].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature2.v)], [signature.r, signature2.r], [signature.s, signature2.s])).to.emit(inboundChannel, "BatchDispatched").withArgs(
      7,
      peers[0].address,
      0, // false result
      1,
      79142, // gas used
      109515633
    );
    expect(await inboundChannel.peersCount()).to.be.equal(2);
  });

  it("should_not_add_a_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("addPeerByPeer", [peers[1].address])
    const batch = {
      nonce: 8,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    let signature2 = ethers.utils.splitSignature(await peers[1].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature2.v)], [signature.r, signature2.r], [signature.s, signature2.s])).to.emit(inboundChannel, "BatchDispatched").withArgs(
      8,
      peers[0].address,
      0, // false result
      1,
      77099, // gas used
      95950261 // base fee
    );
    expect(await inboundChannel.peersCount()).to.be.equal(2);
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
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.revertedWith('not enough signatures');
  });

  it("should_remove_another_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[1].address])
    const batch = {
      nonce: 9,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    let signature2 = ethers.utils.splitSignature(await peers[1].signMessage(ethers.utils.arrayify(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(2);
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature2.v)], [signature.r, signature2.r], [signature.s, signature2.s]);
    expect(await inboundChannel.peersCount()).to.be.equal(1);
    expect(await inboundChannel.isPeer(peers[1].address)).to.be.equal(false);
  });

  it("should_not_remove_last_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("removePeerByPeer", [peers[0].address])
    const batch = {
      nonce: 10,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1000000,
        payload: payload,
      },
      ]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    expect(await inboundChannel.peersCount()).to.be.equal(1);
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s]);
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }

    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.rejectedWith('insufficient gas for delivery of all messages')
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }
    for (let i = 0; i < 256; i++) {
      batch.messages.push(batch.messages[0])
    }
    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.rejectedWith('must be < 256 messages in the batch')
  });

  it("should_revert_on_submit_signed_random_data(huge chunk)", async function () {
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
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }
    for (let i = 0; i < 250; i++) {
      batch.messages.push(batch.messages[0])
    }
    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.emit(inboundChannel, "BatchDispatched").withArgs(
      11,
      peers[0].address,
      "14474011154664524427946373126085988481658748083205070504932198000989141204987", // false result
      253,
      885837, // gas used
      33246817
    );
  });

  it("should_revert_on_submit_signed_random_data(huge chunk)", async function () {
    let batch = {
      nonce: 12,
      total_max_gas: 1000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1,
        payload: "0x00aaff",
      },
      {
        target: inboundChannel.address,
        max_gas: 2,
        payload: "0x00bbff",
      },
      {
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0x00ccff",
      }]
    }
    for (let i = 0; i < 250; i++) {
      batch.messages.push(batch.messages[0])
    }
    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.emit(inboundChannel, "BatchDispatched").withArgs(
      12,
      peers[0].address,
      0, // false result
      253,
      886130, // gas used
      25749367
    );
  });

  it("reverts_on_submit_signed_random_data(Transaction gas limit exceeds block gas limit of 30000000)", async function () {
    let batch = {
      nonce: 13,
      total_max_gas: 20000000,
      messages: [{
        target: inboundChannel.address,
        max_gas: 1,
        payload: "0xe07bc27e9b5ec4da29ece7c092db9c1d93331db1e3836d7d3c2a8e4efdd45126",
      },
      {
        target: inboundChannel.address,
        max_gas: 2,
        payload: "0xe07bc27e9b5ec4da29ece7c092db9c1d93331db1e3836d7d3c2a8e4efdd45126",
      },
      {
        target: inboundChannel.address,
        max_gas: 3,
        payload: "0xe07bc27e9b5ec4da29ece7c092db9c1d93331db1e3836d7d3c2a8e4efdd45126",
      }]
    }
    for (let i = 0; i < 250; i++) {
      batch.messages.push(batch.messages[0])
    }
    let encodedMessage = ethers.utils.keccak256(coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    // console.log("packedMessage:", coder.encode(["tuple(uint nonce, uint total_max_gas, tuple(address target, uint max_gas, bytes payload)[] messages)"], [batch]))
    console.log("encodedMessage:", encodedMessage)
    let signature = ethers.utils.splitSignature(await peers[0].signMessage(ethers.utils.arrayify(encodedMessage)));
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.not.fulfilled;
  });

});
