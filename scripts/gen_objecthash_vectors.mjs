// One-off dev tool (nothing here is imported by the crate): generates the
// object-hash parity vectors committed at tests/vectors/objecthash.json.
//
// Usage (run from a scratch dir OUTSIDE this repo tree - this repo sits inside
// the monkeytype monorepo, and npm walking up to its workspace root fails on
// `workspace:*` deps; ESM import resolution also needs node_modules next to
// the executing script):
//   mkdir /tmp/objecthash-gen && cd /tmp/objecthash-gen
//   echo '{"name":"gen","private":true}' > package.json
//   cp <repo>/scripts/gen_objecthash_vectors.mjs .
//   npm install --no-save object-hash@3.0.0
//   node gen_objecthash_vectors.mjs > <repo>/tests/vectors/objecthash.json
//
// object-hash@3.0.0 is the exact version pinned by the monkeytype frontend and
// backend; the Rust port in src/objecthash.rs must match it byte for byte.
// Each vector records the value, its writeToStream serialization (so a Rust
// mismatch is diagnosable at the exact byte), and the sha1-hex hash.
//
// Vectors are declared as raw JSON text and JSON.parsed so that both sides
// (this script and the Rust test) hash the value parsed from the same text -
// sidestepping representational drift like 1.0 vs 1.

import objectHash from 'object-hash';

const vectors = [
  ['number_zero', '0'],
  ['number_one', '1'],
  ['number_negative_one', '-1'],
  ['number_one_point_zero', '1.0'],
  ['number_decimal', '92.5'],
  ['number_negative_decimal', '-92.5'],
  ['number_duration_boundary', '122.01'],
  ['number_float_artifact', '0.30000000000000004'],
  ['number_tiny_exponent', '1e-7'],
  ['number_smallest_positional', '0.000001'],
  ['number_large_positional', '1e20'],
  ['number_first_exponential', '1e21'],
  ['number_max_safe_plus_one', '9007199254740992'],
  ['number_u64_max', '18446744073709551615'],
  ['string_empty', '""'],
  ['string_hello', '"hello"'],
  ['string_unicode', '"héllo😀"'],
  ['string_grammar_chars', '"with:colons,and,commas"'],
  ['bool_true', 'true'],
  ['bool_false', 'false'],
  ['null', 'null'],
  ['array_empty', '[]'],
  ['object_empty', '{}'],
  ['array_numbers', '[1,2,3]'],
  ['array_of_objects', '[{"a":1},{"b":[2,3]},{}]'],
  ['object_key_sort', '{"Z":1,"a":2,"10":3,"9":4}'],
  ['object_nested_deep', '{"outer":{"mid":{"inner":[1,{"leaf":null}]}}}'],
  ['object_empty_containers', '{"arr":[],"obj":{},"s":""}'],
  [
    'completed_event_time30',
    JSON.stringify({
      acc: 97.62,
      afkDuration: 0,
      bailedOut: false,
      blindMode: false,
      charStats: [148, 3, 1, 2],
      charTotal: 152,
      chartData: {
        wpm: [58, 61, 60, 62, 63, 61, 60, 62, 61, 60, 61, 62, 60, 59, 61, 62, 61, 60, 62, 61, 60, 61, 62, 61, 60, 61, 62, 61, 60, 61],
        burst: [60, 64, 58, 66, 62, 60, 58, 64, 62, 58, 62, 64, 56, 54, 62, 64, 62, 58, 64, 62, 58, 62, 64, 62, 58, 62, 64, 62, 58, 62],
        err: [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      },
      consistency: 84.32,
      difficulty: 'normal',
      funbox: [],
      incompleteTestSeconds: 0,
      incompleteTests: [],
      keyConsistency: 71.18,
      keyDuration: [95.2, 88.7, 102.3, 91.1, 87.9],
      keyOverlap: 12.34,
      keySpacing: [201.5, 189.99, 210.01, 195.3, 188.8],
      language: 'english',
      lastKeyToEnd: 152.34,
      lazyMode: false,
      mode: 'time',
      mode2: '30',
      numbers: false,
      punctuation: false,
      rawWpm: 61.2,
      restartCount: 0,
      startToFirstKey: 0,
      stopOnLetter: false,
      tags: [],
      testDuration: 30,
      timestamp: 1751846400000,
      uid: 'AbCdEf0123456789AbCdEf0123456789',
      wpm: 59.19,
      wpmConsistency: 88.5,
    }),
  ],
  [
    'completed_event_words25',
    JSON.stringify({
      acc: 100,
      afkDuration: 0,
      bailedOut: false,
      blindMode: false,
      charStats: [130, 0, 0, 0],
      charTotal: 130,
      chartData: {
        wpm: [55, 57, 58, 57, 58, 59, 58, 58, 59, 58, 59, 58, 59, 58, 59, 58, 59, 58, 59, 58, 59, 58, 59, 58, 59, 58],
        burst: [55, 59, 60, 54, 62, 63, 52, 58, 63, 50, 63, 52, 63, 52, 63, 52, 63, 52, 63, 52, 63, 52, 63, 52, 63, 52],
        err: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      },
      consistency: 90.11,
      difficulty: 'normal',
      funbox: [],
      incompleteTestSeconds: 0,
      incompleteTests: [],
      keyConsistency: 75.4,
      keyDuration: [],
      keyOverlap: 0,
      keySpacing: [180.1, 190.25, 200.4],
      language: 'english',
      lastKeyToEnd: 98.7,
      lazyMode: false,
      mode: 'words',
      mode2: '25',
      numbers: false,
      punctuation: false,
      rawWpm: 58,
      restartCount: 0,
      startToFirstKey: 0,
      stopOnLetter: false,
      tags: [],
      testDuration: 26.91,
      timestamp: 1751846500000,
      uid: 'AbCdEf0123456789AbCdEf0123456789',
      wpm: 58,
      wpmConsistency: 91.02,
    }),
  ],
  [
    'completed_event_toolong',
    JSON.stringify({
      acc: 95.5,
      afkDuration: 2,
      bailedOut: false,
      blindMode: false,
      charStats: [610, 12, 4, 6],
      charTotal: 626,
      chartData: 'toolong',
      consistency: 80.02,
      difficulty: 'normal',
      funbox: [],
      incompleteTestSeconds: 0,
      incompleteTests: [],
      keyConsistency: 69.99,
      keyDuration: 'toolong',
      keyOverlap: 44.4,
      keySpacing: 'toolong',
      language: 'english',
      lastKeyToEnd: 310.5,
      lazyMode: false,
      mode: 'words',
      mode2: '100',
      numbers: false,
      punctuation: false,
      rawWpm: 62.3,
      restartCount: 0,
      startToFirstKey: 0,
      stopOnLetter: false,
      tags: [],
      testDuration: 123.45,
      timestamp: 1751846600000,
      uid: 'AbCdEf0123456789AbCdEf0123456789',
      wpm: 60.75,
      wpmConsistency: 85,
    }),
  ],
];

function serialize(value) {
  let buf = '';
  objectHash.writeToStream(value, {
    write(chunk) {
      buf += chunk;
    },
  });
  return buf;
}

const out = vectors.map(([name, text]) => {
  const value = JSON.parse(text);
  return {
    name,
    value,
    serialized: serialize(value),
    hash: objectHash(value),
  };
});

process.stdout.write(JSON.stringify(out, null, 2) + '\n');
