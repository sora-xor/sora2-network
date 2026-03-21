import { expect } from 'chai';
import { network } from 'hardhat';

async function expectCustomError(promise, contract, name) {
  try {
    await promise;
    throw new Error('expected revert');
  } catch (e) {
    const data = e?.data ?? e?.error?.data ?? e?.info?.error?.data;
    if (!data) {
      throw e;
    }
    const decoded = contract.interface.parseError(data);
    expect(decoded?.name).to.equal(name);
  }
}

async function expectPanic(promise, code) {
  try {
    await promise;
    throw new Error('expected panic');
  } catch (e) {
    const data = e?.data ?? e?.error?.data ?? e?.info?.error?.data;
    if (!data || !data.startsWith('0x4e487b71')) {
      throw e;
    }
    if (code !== undefined) {
      const abi = data.slice(10);
      const decoded = BigInt(`0x${abi}`);
      expect(decoded).to.equal(code);
    }
  }
}

describe('SCCP (EVM) token', function () {
  it('rejects zero-address router in constructor', async function () {
    const { ethers } = await network.connect();
    const Token = await ethers.getContractFactory('SccpToken');
    const contractLike = { interface: Token.interface };

    await expectCustomError(
      Token.deploy('SCCP Wrapped', 'wSORA', 18, ethers.ZeroAddress),
      contractLike,
      'ZeroAddress',
    );
  });

  it('enforces only-router minting and zero-address mint recipients', async function () {
    const { ethers } = await network.connect();
    const [router, user] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await expectCustomError(token.connect(user).mint(user.address, 1n), token, 'OnlyRouter');
    await expectCustomError(token.connect(router).mint(ethers.ZeroAddress, 1n), token, 'ZeroAddress');

    await (await token.connect(router).mint(user.address, 7n)).wait();
    expect(await token.balanceOf(user.address)).to.equal(7n);
    expect(await token.totalSupply()).to.equal(7n);
  });

  it('checks transfer and transferFrom balance and allowance edges', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob, carol] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 10n)).wait();

    await expectCustomError(token.connect(alice).transfer(ethers.ZeroAddress, 1n), token, 'ZeroAddress');
    await expectCustomError(token.connect(alice).transfer(bob.address, 11n), token, 'InsufficientBalance');

    await (await token.connect(alice).approve(bob.address, 3n)).wait();
    await (await token.connect(bob).transferFrom(alice.address, carol.address, 2n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(1n);
    expect(await token.balanceOf(alice.address)).to.equal(8n);
    expect(await token.balanceOf(carol.address)).to.equal(2n);

    await expectCustomError(
      token.connect(bob).transferFrom(alice.address, carol.address, 2n),
      token,
      'InsufficientAllowance',
    );

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    await (await token.connect(bob).transferFrom(alice.address, carol.address, 1n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(ethers.MaxUint256);
  });

  it('burn and burnFrom preserve supply invariants and allowance semantics', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 9n)).wait();

    await (await token.connect(alice).burn(4n)).wait();
    expect(await token.balanceOf(alice.address)).to.equal(5n);
    expect(await token.totalSupply()).to.equal(5n);

    await expectCustomError(token.connect(bob).burnFrom(alice.address, 1n), token, 'InsufficientAllowance');

    await (await token.connect(alice).approve(bob.address, 2n)).wait();
    await (await token.connect(bob).burnFrom(alice.address, 2n)).wait();
    expect(await token.balanceOf(alice.address)).to.equal(3n);
    expect(await token.totalSupply()).to.equal(3n);
    expect(await token.allowance(alice.address, bob.address)).to.equal(0n);

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    await (await token.connect(bob).burnFrom(alice.address, 1n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(ethers.MaxUint256);
    expect(await token.balanceOf(alice.address)).to.equal(2n);
    expect(await token.totalSupply()).to.equal(2n);

    await expectCustomError(token.connect(alice).burn(3n), token, 'InsufficientBalance');
  });

  it('emits Approval on finite allowance spend and skips Approval on infinite allowance spend', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob, carol] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();
    const tokenAddress = (await token.getAddress()).toLowerCase();
    const approvalTopic = token.interface.getEvent('Approval').topicHash;
    const approvalLogs = (receipt) =>
      receipt.logs.filter(
        (log) =>
          log.address.toLowerCase() === tokenAddress &&
          log.topics.length > 0 &&
          log.topics[0] === approvalTopic,
      );

    await (await token.connect(router).mint(alice.address, 10n)).wait();

    await (await token.connect(alice).approve(bob.address, 3n)).wait();
    {
      const tx = await token.connect(bob).transferFrom(alice.address, carol.address, 1n);
      const receipt = await tx.wait();
      const logs = approvalLogs(receipt);
      expect(logs.length).to.equal(1);
      const decoded = token.interface.decodeEventLog('Approval', logs[0].data, logs[0].topics);
      expect(decoded.owner).to.equal(alice.address);
      expect(decoded.spender).to.equal(bob.address);
      expect(decoded.value).to.equal(2n);
    }

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    {
      const tx = await token.connect(bob).transferFrom(alice.address, carol.address, 1n);
      const receipt = await tx.wait();
      expect(approvalLogs(receipt).length).to.equal(0);
    }

    await (await token.connect(alice).approve(bob.address, 4n)).wait();
    {
      const tx = await token.connect(bob).burnFrom(alice.address, 1n);
      const receipt = await tx.wait();
      const logs = approvalLogs(receipt);
      expect(logs.length).to.equal(1);
      const decoded = token.interface.decodeEventLog('Approval', logs[0].data, logs[0].topics);
      expect(decoded.owner).to.equal(alice.address);
      expect(decoded.spender).to.equal(bob.address);
      expect(decoded.value).to.equal(3n);
    }

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    {
      const tx = await token.connect(bob).burnFrom(alice.address, 1n);
      const receipt = await tx.wait();
      expect(approvalLogs(receipt).length).to.equal(0);
    }
  });

  it('does not consume allowance when transferFrom or burnFrom reverts', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 2n)).wait();

    await (await token.connect(alice).approve(bob.address, 2n)).wait();
    await expectCustomError(
      token.connect(bob).transferFrom(alice.address, ethers.ZeroAddress, 1n),
      token,
      'ZeroAddress',
    );
    expect(await token.allowance(alice.address, bob.address)).to.equal(2n);
    expect(await token.balanceOf(alice.address)).to.equal(2n);

    await (await token.connect(alice).approve(bob.address, 3n)).wait();
    await expectCustomError(
      token.connect(bob).burnFrom(alice.address, 3n),
      token,
      'InsufficientBalance',
    );
    expect(await token.allowance(alice.address, bob.address)).to.equal(3n);
    expect(await token.balanceOf(alice.address)).to.equal(2n);
    expect(await token.totalSupply()).to.equal(2n);

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    await expectCustomError(
      token.connect(bob).transferFrom(alice.address, ethers.ZeroAddress, 1n),
      token,
      'ZeroAddress',
    );
    expect(await token.allowance(alice.address, bob.address)).to.equal(ethers.MaxUint256);

    await expectCustomError(
      token.connect(bob).burnFrom(alice.address, 3n),
      token,
      'InsufficientBalance',
    );
    expect(await token.allowance(alice.address, bob.address)).to.equal(ethers.MaxUint256);
  });

  it('does not consume allowance when transferFrom reverts for insufficient balance', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob, carol] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 1n)).wait();

    await (await token.connect(alice).approve(bob.address, 2n)).wait();
    await expectCustomError(
      token.connect(bob).transferFrom(alice.address, carol.address, 2n),
      token,
      'InsufficientBalance',
    );
    expect(await token.allowance(alice.address, bob.address)).to.equal(2n);
    expect(await token.balanceOf(alice.address)).to.equal(1n);
    expect(await token.balanceOf(carol.address)).to.equal(0n);

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    await expectCustomError(
      token.connect(bob).transferFrom(alice.address, carol.address, 2n),
      token,
      'InsufficientBalance',
    );
    expect(await token.allowance(alice.address, bob.address)).to.equal(ethers.MaxUint256);
    expect(await token.balanceOf(alice.address)).to.equal(1n);
    expect(await token.balanceOf(carol.address)).to.equal(0n);
  });

  it('zero-value transferFrom and burnFrom keep balances, supply, and allowance stable', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob, carol] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 5n)).wait();

    await expectCustomError(
      token.connect(bob).transferFrom(alice.address, ethers.ZeroAddress, 0n),
      token,
      'ZeroAddress',
    );

    await (await token.connect(alice).approve(bob.address, 0n)).wait();
    await (await token.connect(bob).transferFrom(alice.address, carol.address, 0n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(0n);
    expect(await token.balanceOf(alice.address)).to.equal(5n);
    expect(await token.balanceOf(carol.address)).to.equal(0n);

    await (await token.connect(bob).burnFrom(alice.address, 0n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(0n);
    expect(await token.balanceOf(alice.address)).to.equal(5n);
    expect(await token.totalSupply()).to.equal(5n);

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    await (await token.connect(bob).transferFrom(alice.address, carol.address, 0n)).wait();
    await (await token.connect(bob).burnFrom(alice.address, 0n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(ethers.MaxUint256);
    expect(await token.balanceOf(alice.address)).to.equal(5n);
    expect(await token.balanceOf(carol.address)).to.equal(0n);
    expect(await token.totalSupply()).to.equal(5n);
  });

  it('self-transfer and self-transferFrom keep balances and totalSupply unchanged', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 10n)).wait();

    await (await token.connect(alice).transfer(alice.address, 7n)).wait();
    expect(await token.balanceOf(alice.address)).to.equal(10n);
    expect(await token.totalSupply()).to.equal(10n);

    await (await token.connect(alice).approve(bob.address, 4n)).wait();
    await (await token.connect(bob).transferFrom(alice.address, alice.address, 4n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(0n);
    expect(await token.balanceOf(alice.address)).to.equal(10n);
    expect(await token.totalSupply()).to.equal(10n);

    await (await token.connect(alice).approve(bob.address, ethers.MaxUint256)).wait();
    await (await token.connect(bob).transferFrom(alice.address, alice.address, 3n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(ethers.MaxUint256);
    expect(await token.balanceOf(alice.address)).to.equal(10n);
    expect(await token.totalSupply()).to.equal(10n);
  });

  it('zero-value transfer and burn are no-op and preserve balances/supply', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 9n)).wait();

    await (await token.connect(alice).transfer(bob.address, 0n)).wait();
    await (await token.connect(alice).burn(0n)).wait();
    await (await token.connect(bob).burn(0n)).wait();

    expect(await token.balanceOf(alice.address)).to.equal(9n);
    expect(await token.balanceOf(bob.address)).to.equal(0n);
    expect(await token.totalSupply()).to.equal(9n);
  });

  it('zero-value transfer to zero address still reverts and preserves state', async function () {
    const { ethers } = await network.connect();
    const [router, alice] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 5n)).wait();
    const supplyBefore = await token.totalSupply();
    const balanceBefore = await token.balanceOf(alice.address);

    await expectCustomError(
      token.connect(alice).transfer(ethers.ZeroAddress, 0n),
      token,
      'ZeroAddress',
    );
    expect(await token.totalSupply()).to.equal(supplyBefore);
    expect(await token.balanceOf(alice.address)).to.equal(balanceBefore);
  });

  it('zero-value transferFrom and burnFrom are no-op without prior approval', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob, carol] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 7n)).wait();

    expect(await token.allowance(alice.address, bob.address)).to.equal(0n);
    expect(await token.allowance(carol.address, bob.address)).to.equal(0n);

    await (await token.connect(bob).transferFrom(alice.address, carol.address, 0n)).wait();
    await (await token.connect(bob).burnFrom(alice.address, 0n)).wait();
    await (await token.connect(bob).transferFrom(carol.address, alice.address, 0n)).wait();
    await (await token.connect(bob).burnFrom(carol.address, 0n)).wait();

    expect(await token.balanceOf(alice.address)).to.equal(7n);
    expect(await token.balanceOf(carol.address)).to.equal(0n);
    expect(await token.totalSupply()).to.equal(7n);
    expect(await token.allowance(alice.address, bob.address)).to.equal(0n);
    expect(await token.allowance(carol.address, bob.address)).to.equal(0n);
  });

  it('allows approving the zero-address spender and updates allowance deterministically', async function () {
    const { ethers } = await network.connect();
    const [router] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).approve(ethers.ZeroAddress, 7n)).wait();
    expect(await token.allowance(router.address, ethers.ZeroAddress)).to.equal(7n);

    await (await token.connect(router).approve(ethers.ZeroAddress, 0n)).wait();
    expect(await token.allowance(router.address, ethers.ZeroAddress)).to.equal(0n);
  });

  it('approve overwrites existing allowance instead of adding to it', async function () {
    const { ethers } = await network.connect();
    const [router, alice, bob] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, 10n)).wait();
    await (await token.connect(alice).approve(bob.address, 5n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(5n);

    await (await token.connect(alice).approve(bob.address, 2n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(2n);

    await (await token.connect(bob).transferFrom(alice.address, bob.address, 1n)).wait();
    expect(await token.allowance(alice.address, bob.address)).to.equal(1n);
  });

  it('emits Approval event when approving a zero-address spender', async function () {
    const { ethers } = await network.connect();
    const [owner] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, owner.address);
    await token.waitForDeployment();

    const approvalTopic = token.interface.getEvent('Approval').topicHash;
    const tx = await token.connect(owner).approve(ethers.ZeroAddress, 9n);
    const receipt = await tx.wait();
    const logs = receipt.logs.filter((log) => log.address === token.target && log.topics[0] === approvalTopic);

    expect(logs.length).to.equal(1);
    const decoded = token.interface.decodeEventLog('Approval', logs[0].data, logs[0].topics);
    expect(decoded.owner).to.equal(owner.address);
    expect(decoded.spender).to.equal(ethers.ZeroAddress);
    expect(decoded.value).to.equal(9n);
    expect(await token.allowance(owner.address, ethers.ZeroAddress)).to.equal(9n);
  });

  it('emits Approval when approving the same spender/value repeatedly', async function () {
    const { ethers } = await network.connect();
    const [owner, spender] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, owner.address);
    await token.waitForDeployment();

    const approvalTopic = token.interface.getEvent('Approval').topicHash;

    const tx1 = await token.connect(owner).approve(spender.address, 3n);
    const receipt1 = await tx1.wait();
    const logs1 = receipt1.logs.filter((log) => log.address === token.target && log.topics[0] === approvalTopic);
    expect(logs1.length).to.equal(1);
    expect(await token.allowance(owner.address, spender.address)).to.equal(3n);

    const tx2 = await token.connect(owner).approve(spender.address, 3n);
    const receipt2 = await tx2.wait();
    const logs2 = receipt2.logs.filter((log) => log.address === token.target && log.topics[0] === approvalTopic);
    expect(logs2.length).to.equal(1);
    expect(await token.allowance(owner.address, spender.address)).to.equal(3n);
  });

  it('fails closed on mint overflow and preserves balances and supply', async function () {
    const { ethers } = await network.connect();
    const [router, alice] = await ethers.getSigners();

    const Token = await ethers.getContractFactory('SccpToken');
    const token = await Token.deploy('SCCP Wrapped', 'wSORA', 18, router.address);
    await token.waitForDeployment();

    await (await token.connect(router).mint(alice.address, ethers.MaxUint256)).wait();
    expect(await token.totalSupply()).to.equal(ethers.MaxUint256);
    expect(await token.balanceOf(alice.address)).to.equal(ethers.MaxUint256);

    await expectPanic(token.connect(router).mint(alice.address, 1n), 0x11n);
    expect(await token.totalSupply()).to.equal(ethers.MaxUint256);
    expect(await token.balanceOf(alice.address)).to.equal(ethers.MaxUint256);
  });
});
