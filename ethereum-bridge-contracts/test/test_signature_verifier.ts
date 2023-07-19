import { ethers } from 'hardhat';
import { expect } from 'chai';
import { SignerWithAddress } from '@nomiclabs/hardhat-ethers/signers';
import { InboundChannel } from '../typechain/contracts/InboundChannel';
import { OutboundChannel } from '../typechain/contracts/OutboundChannel';
import { InboundChannel__factory } from '../typechain/factories/contracts/InboundChannel__factory';
import { OutboundChannel__factory } from '../typechain/factories/contracts/OutboundChannel__factory';

describe("Signature Verifier", function () {
  let peers: SignerWithAddress[]
  let inboundChannel: InboundChannel;
  let inboundFactory: InboundChannel__factory;
  let outboundFactory: OutboundChannel__factory;
  let outboundChannel: OutboundChannel;
  const coder = ethers.utils.defaultAbiCoder

  beforeEach(async function () {
    peers = await ethers.getSigners();
    inboundFactory = (await ethers.getContractFactory('InboundChannel')) as InboundChannel__factory;
    outboundFactory = (await ethers.getContractFactory('OutboundChannel')) as OutboundChannel__factory;
    inboundChannel = await (await inboundFactory.deploy()).deployed()
    outboundChannel = await (await outboundFactory.deploy()).deployed()
    await inboundChannel.initialize(outboundChannel.address, [peers[0].address])
  });

  it("should_submit_signed_random_data", async function () {
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
    await inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])
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
    batch.nonce = 2; 
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.revertedWith("Invalid signatures");
  });

  it("should_revert_on_invalid_signature", async function () {
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
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v).add(3)], [signature.r], [signature.s])).to.be.revertedWith("ECDSA: invalid signature 'v' value");
  });

  it("should_revert_on_invalid_signature_v_length_mismatch", async function () {
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
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature.v)], [signature.r], [signature.s])).to.be.revertedWith("v and r length mismatch");
  });

  it("should_revert_on_invalid_signature_s_length_mismatch", async function () {
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
    await expect(inboundChannel.submit(batch, [ethers.BigNumber.from(signature.v), ethers.BigNumber.from(signature.v)], [signature.r, signature.r], [signature.s])).to.be.revertedWith("v and s length mismatch");
  });

  it("should_add_a_peer", async function () {
    const payload = inboundChannel.interface.encodeFunctionData("addPeerByPeer", [peers[1].address])
    const batch = {
      nonce: 1,
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

});
