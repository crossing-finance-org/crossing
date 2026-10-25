/** Independent BigInt auction reference; valid JavaScript and TypeScript syntax.
 * Reads one JSON book per line; integer amounts are decimal strings.
 * Does not import/translate Python's resumable walker or settlement model.
 * This is not the SDK and does not implement Soroban commitment XDR.
 */
import { createInterface } from 'node:readline';

function evaluate(input) {
  const c = input.config;
  const sale = c.mode === 'sale';
  const supply = BigInt(c.supply), scale = BigInt(c.scale);
  const floor = BigInt(c.min_price), minimum = BigInt(c.min_fill);
  const bids = input.bids.map(b => ({
    active: b.quantity !== null && !b.excluded,
    p: BigInt(b.price ?? 0), q: BigInt(b.quantity ?? 0),
  }));
  const book = bids.filter(b => b.active);
  const prices = [...new Set(book.map(b => b.p.toString()))]
    .map(BigInt).sort((a, b) => a < b ? -1 : a > b ? 1 : 0);
  let price = null, numerator = 1n, denominator = 1n;
  if (sale) {
    // Find the marginal bid in a sorted individual book, rather than walking
    // an aggregated grid. Then derive its entire equal-price rationing bucket.
    const ordered = [...book].sort((a, b) => a.p > b.p ? -1 : a.p < b.p ? 1 : 0);
    const demand = ordered.reduce((n, b) => n+b.q, 0n);
    if (demand > 0n && demand < supply) price = floor;
    else if (demand >= supply) {
      let consumed = 0n;
      for (const bid of ordered) {
        consumed += bid.q;
        if (consumed >= supply) { price = bid.p; break; }
      }
      const better = book.filter(b => b.p > price).reduce((n, b) => n+b.q, 0n);
      numerator = supply-better;
      denominator = book.filter(b => b.p === price).reduce((n, b) => n+b.q, 0n);
    }
  } else {
    // Evaluate each occupied-price feasibility row from the entire book.
    // Do not maintain the Python walker's cursor or cumulative state.
    for (let index = 0; index < prices.length; index++) {
      const p = prices[index];
      const better = book.filter(b => b.p < p).reduce((n, b) => n+b.q, 0n);
      const at = book.filter(b => b.p === p).reduce((n, b) => n+b.q, 0n);
      const capacity = supply*scale/p;
      if (capacity <= better) {
        if (better > 0n) price = prices[index-1];
        break;
      }
      if (better+at >= capacity) {
        price = p; numerator = capacity-better; denominator = at;
        break;
      }
      price = p;
    }
  }
  let allocations = bids.map(b => {
    if (!b.active || price === null) return 0n;
    const better = sale ? b.p > price : b.p < price;
    if (better) return b.q;
    return b.p === price ? b.q*numerator/denominator : 0n;
  });
  let payments = allocations.map(q => {
    if (price === null) return 0n;
    const product = q*price;
    return product/scale + (sale && product%scale !== 0n ? 1n : 0n);
  });
  allocations = allocations.map((q, i) => payments[i] === 0n ? 0n : q);
  const filled = allocations.reduce((n, q) => n+q, 0n);
  if (filled === 0n || filled < minimum) {
    price = null;
    allocations = bids.map(() => 0n); payments = bids.map(() => 0n);
  }
  return { price: price === null ? null : price.toString(),
    allocations: allocations.map(String), payments: payments.map(String) };
}

for await (const line of createInterface({ input: process.stdin, crlfDelay: Infinity })) {
  if (line.trim()) process.stdout.write(JSON.stringify(evaluate(JSON.parse(line)))+'\n');
}
