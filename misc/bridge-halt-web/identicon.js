// Polkadot identicon: the circle avatar that wallets and Polkaswap show for an
// account. The algorithm is ported unchanged from @polkadot/ui-shared 3.16.7,
// icons/polkadot.js (Copyright @polkadot/ui-shared authors & contributors,
// Apache-2.0), so an account looks the same here as in the guardian's wallet.
(() => {
  'use strict';

  const { blake2AsU8a, decodeAddress } = window.polkadotUtilCrypto;

  const S = 64;
  const C = S / 2;
  const Z = (S / 64) * 5;
  const SCHEMES = [
    /* target  */ { colors: [0, 28, 0, 0, 28, 0, 0, 28, 0, 0, 28, 0, 0, 28, 0, 0, 28, 0, 1], freq: 1 },
    /* cube    */ { colors: [0, 1, 3, 2, 4, 3, 0, 1, 3, 2, 4, 3, 0, 1, 3, 2, 4, 3, 5], freq: 20 },
    /* quazar  */ { colors: [1, 2, 3, 1, 2, 4, 5, 5, 4, 1, 2, 3, 1, 2, 4, 5, 5, 4, 0], freq: 16 },
    /* flower  */ { colors: [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 3], freq: 32 },
    /* cyclic  */ { colors: [0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5, 6], freq: 32 },
    /* vmirror */ { colors: [0, 1, 2, 3, 4, 5, 3, 4, 2, 0, 1, 6, 7, 8, 9, 7, 8, 6, 10], freq: 128 },
    /* hmirror */ { colors: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 8, 6, 7, 5, 3, 4, 2, 11], freq: 128 },
  ];
  const SCHEMES_TOTAL = SCHEMES.map((s) => s.freq).reduce((a, b) => a + b);
  const OUTER_CIRCLE = { cx: C, cy: C, fill: '#eee', r: C };
  let zeroHash = new Uint8Array();

  function getRotation(isSixPoint) {
    const r = isSixPoint ? (C / 8) * 5 : (C / 4) * 3;
    const rroot3o2 = (r * Math.sqrt(3)) / 2;
    const ro2 = r / 2;
    const rroot3o4 = (r * Math.sqrt(3)) / 4;
    const ro4 = r / 4;
    const r3o4 = (r * 3) / 4;
    return { r, r3o4, ro2, ro4, rroot3o2, rroot3o4 };
  }

  function getCircleXY(isSixPoint = false) {
    const { r, r3o4, ro2, ro4, rroot3o2, rroot3o4 } = getRotation(isSixPoint);
    return [
      [C, C - r],
      [C, C - ro2],
      [C - rroot3o4, C - r3o4],
      [C - rroot3o2, C - ro2],
      [C - rroot3o4, C - ro4],
      [C - rroot3o2, C],
      [C - rroot3o2, C + ro2],
      [C - rroot3o4, C + ro4],
      [C - rroot3o4, C + r3o4],
      [C, C + r],
      [C, C + ro2],
      [C + rroot3o4, C + r3o4],
      [C + rroot3o2, C + ro2],
      [C + rroot3o4, C + ro4],
      [C + rroot3o2, C],
      [C + rroot3o2, C - ro2],
      [C + rroot3o4, C - ro4],
      [C + rroot3o4, C - r3o4],
      [C, C],
    ];
  }

  function findScheme(d) {
    let cum = 0;
    const schema = SCHEMES.find((item) => {
      cum += item.freq;
      return d < cum;
    });
    if (!schema) {
      throw new Error('Unable to find schema');
    }
    return schema;
  }

  function addressToId(address) {
    if (!zeroHash.length) {
      zeroHash = blake2AsU8a(new Uint8Array(32), 512);
    }
    return blake2AsU8a(decodeAddress(address), 512).map((x, i) => (x + 256 - zeroHash[i]) % 256);
  }

  function getColors(address) {
    const id = addressToId(address);
    const d = Math.floor((id[30] + id[31] * 256) % SCHEMES_TOTAL);
    const rot = (id[28] % 6) * 3;
    const sat = (Math.floor((id[29] * 70) / 256 + 26) % 80) + 30;
    const scheme = findScheme(d);
    const palette = Array.from(id).map((x, i) => {
      const b = (x + (i % 28) * 58) % 256;
      if (b === 0) {
        return '#444';
      } else if (b === 255) {
        return 'transparent';
      }
      const h = Math.floor(((b % 64) * 360) / 64);
      const l = [53, 15, 35, 75][Math.floor(b / 64)];
      return `hsl(${h}, ${sat}%, ${l}%)`;
    });
    return scheme.colors.map((_, i) => palette[scheme.colors[i < 18 ? (i + rot) % 18 : 18]]);
  }

  function polkadotIcon(address, { isAlternative }) {
    const xy = getCircleXY(isAlternative);
    let colors;
    try {
      colors = getColors(address);
    } catch {
      colors = new Array(xy.length).fill('#ddd');
    }
    return [OUTER_CIRCLE].concat(xy.map(([cx, cy], index) => ({ cx, cy, fill: colors[index], r: Z })));
  }

  const SVG = 'http://www.w3.org/2000/svg';

  /// An <svg> element showing the identicon of `address`, `size` pixels square.
  function identicon(address, size) {
    const svg = document.createElementNS(SVG, 'svg');
    svg.setAttribute('viewBox', `0 0 ${S} ${S}`);
    svg.setAttribute('width', String(size));
    svg.setAttribute('height', String(size));
    svg.setAttribute('aria-hidden', 'true');
    for (const { cx, cy, fill, r } of polkadotIcon(address, { isAlternative: false })) {
      const circle = document.createElementNS(SVG, 'circle');
      circle.setAttribute('cx', String(cx));
      circle.setAttribute('cy', String(cy));
      circle.setAttribute('r', String(r));
      circle.setAttribute('fill', fill);
      svg.appendChild(circle);
    }
    return svg;
  }

  window.bridgeHaltIdenticon = identicon;
})();
