/** Independent canonical ScVal XDR encoder: no Stellar SDK or production code.
 * Discriminants come from stellar-xdr 25.0.0, curr/generated.rs. The matrix is
 * wire conformance; individual mutated configurations need not pass admission.
 * --print preserves the original single-vector output. --print-domains emits
 * the matrix explicitly to stdout; this script never overwrites fixtures.
 */
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';

const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32BE(n); return b; };
const opaque = b => Buffer.concat([u32(b.length), b, Buffer.alloc((4 - b.length % 4) % 4)]);
const symbol = s => Buffer.concat([u32(15), opaque(Buffer.from(s))]);
const binary = b => Buffer.concat([u32(13), opaque(b)]);
const address = value => Buffer.concat([
  u32(18), u32(value.kind === 'account' ? 0 : 1),
  ...(value.kind === 'account' ? [u32(0)] : []), Buffer.from(value.id, 'hex'),
]);
const i128 = n => {
  const b = Buffer.alloc(16), v = BigInt(n);
  assert(v >= -(1n << 127n) && v < (1n << 127n));
  b.writeBigInt64BE(v >> 64n, 0);
  b.writeBigUInt64BE(v & ((1n << 64n) - 1n), 8);
  return Buffer.concat([u32(10), b]);
};
const integer = n => Buffer.concat([u32(3), u32(n)]);
const vector = items => Buffer.concat([u32(16), u32(1), u32(items.length), ...items]);
const hash = b => createHash('sha256').update(b).digest();
const filled = fill => Buffer.alloc(32, fill).toString('hex');
const account = fill => ({kind: 'account', id: filled(fill)});
const contract = fill => ({kind: 'contract', id: filled(fill)});
const base = {
  config: {
    version: 1, rwa_token: contract(0x77), quote_token: contract(0x88),
    issuer: account(0x22), buyback_recipient: account(0x22), mode: 'Sale',
    bid_mode: 'Sealed', nonreveal_penalty_bps: 2500, supply_or_budget: '3',
    min_fill_quantity: '1', min_bid_quantity: '1', min_deposit: '1',
    min_price: '10000000', max_price: '100000000', tick_size: '10000000',
    max_bids: 16, max_bids_per_address: 4, start_ledger: 100, end_ledger: 110,
    reveal_end_ledger: 115, close_deadline: 140, delivery_deadline: 160,
  },
  domain: 'CROSSING_BID_V1', network_id: filled(0x33), auction: contract(0x11),
  owner: account(0x22), nonce: filled(0x55), locked: '12345678901234567890123456789',
  price: '20000000', quantity: '1234567', salt: filled(0x66),
};
const addressFields = new Set(['rwa_token', 'quote_token', 'issuer', 'buyback_recipient']);
const enumFields = new Set(['mode', 'bid_mode']);
const amountFields = new Set([
  'supply_or_budget', 'min_fill_quantity', 'min_bid_quantity', 'min_deposit',
  'min_price', 'max_price', 'tick_size',
]);
function encode(input) {
  const fields = Object.fromEntries(Object.entries(input.config).map(([k, v]) => [k,
    addressFields.has(k) ? address(v) : enumFields.has(k) ? vector([symbol(v)]) :
    amountFields.has(k) ? i128(v) : integer(v),
  ]));
  const configXdr = Buffer.concat([
    u32(17), u32(1), u32(Object.keys(fields).length),
    ...Object.keys(fields).sort().flatMap(k => [symbol(k), fields[k]]),
  ]);
  const configHash = hash(configXdr);
  const boundConfigHash = input.config_hash_override ?
    Buffer.from(input.config_hash_override, 'hex') : configHash;
  const preimage = vector([
    symbol(input.domain), binary(Buffer.from(input.network_id, 'hex')), address(input.auction),
    address(input.owner), binary(Buffer.from(input.nonce, 'hex')), binary(boundConfigHash),
    i128(input.locked), i128(input.price), i128(input.quantity), binary(Buffer.from(input.salt, 'hex')),
  ]);
  return {
    config_hash: configHash.toString('hex'), bound_config_hash: boundConfigHash.toString('hex'),
    commitment: hash(preimage).toString('hex'), preimage_xdr: preimage.toString('hex'),
  };
}
function matrix() {
  const cases = [];
  const add = (name, mutate = () => {}) => {
    const input = structuredClone(base);
    mutate(input);
    cases.push({name, input, expected: encode(input)});
  };
  add('legacy_sale_account_owner');
  add('buyback_contract_owner', input => {
    input.config.mode = 'Buyback'; input.owner = contract(0x22);
  });
  add('amount_high64', input => {
    input.config.supply_or_budget = input.quantity = input.locked = '18446744073709551616';
    input.price = '10000000';
  });
  add('amount_i128_max', input => {
    input.config.supply_or_budget = input.quantity = input.locked = '170141183460469231731687303715884105727';
    input.config.max_price = input.price = '10000000';
  });
  for (const key of Object.keys(base.config)) {
    add(`config_${key}`, input => {
      const value = input.config[key];
      if (addressFields.has(key)) {
        input.config[key].id = filled(Buffer.from(value.id, 'hex')[0] + 1);
      } else if (key === 'mode') input.config[key] = 'Buyback';
      else if (key === 'bid_mode') input.config[key] = 'Open';
      else if (amountFields.has(key)) input.config[key] = (BigInt(value) + 1n).toString();
      else input.config[key] += 1;
    });
  }
  const mutations = {
    domain: input => { input.domain = 'CROSSING_BID_V2'; },
    network: input => { input.network_id = filled(0x34); },
    auction: input => { input.auction = contract(0x12); },
    owner: input => { input.owner = contract(0x22); },
    nonce: input => { input.nonce = filled(0x56); },
    config_hash: input => { input.config_hash_override = filled(0x99); },
    locked: input => { input.locked = (BigInt(input.locked) + 1n).toString(); },
    price: input => { input.price = (BigInt(input.price) + 1n).toString(); },
    quantity: input => { input.quantity = (BigInt(input.quantity) + 1n).toString(); },
    salt: input => { input.salt = filled(0x67); },
  };
  for (const [name, mutate] of Object.entries(mutations)) add(`field_${name}`, mutate);
  assert.equal(cases.length, 36);
  for (const c of cases.slice(4)) assert.notEqual(c.expected.commitment, cases[0].expected.commitment, c.name);
  return {format: 'crossing-commitment-domains-v1', cases};
}
const encodedBase = encode(base);
const legacy = {
  config_hash: encodedBase.config_hash,
  commitment: encodedBase.commitment,
  preimage_xdr: encodedBase.preimage_xdr,
};
if (process.argv.includes('--print-domains')) process.stdout.write(JSON.stringify(matrix(), null, 2) + '\n');
else if (process.argv.includes('--print')) process.stdout.write(JSON.stringify(legacy, null, 2) + '\n');
else {
  const expected = JSON.parse(readFileSync(new URL('../../tests/vectors/commitment-v1.json', import.meta.url), 'utf8'));
  assert.deepEqual(legacy, expected);
  const expectedDomains = JSON.parse(readFileSync(new URL('../../tests/vectors/commitment-domains.json', import.meta.url), 'utf8'));
  const actual = matrix();
  assert.deepEqual(actual, expectedDomains);
  process.stdout.write(`Canonical legacy vector and ${actual.cases.length} independent commitment-domain vectors passed.\n`);
}
