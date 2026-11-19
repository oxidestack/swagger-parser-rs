#!/usr/bin/env python3
"""Writes crates/jackson/tests/fixtures/json-bytes-cases.txt: byte inputs for `readTree(byte[])`, one hex line each
(record the expected side with JsonBytesCases.java)."""
import json, os, sys

cases = []
def add(b):
    cases.append(b if isinstance(b, bytes) else b.encode("utf-8"))

here = os.path.dirname(os.path.abspath(__file__))
fixtures = os.path.join(here, "..", "..", "crates", "jackson", "tests", "fixtures")
# every ASCII/text case of the String corpus, as UTF-8 bytes
for line in open(os.path.join(fixtures, "json-cases.txt"), encoding="utf-8"):
    add(json.loads(line))
for line in open(os.path.join(fixtures, "mockserver-json-cases.txt"), encoding="utf-8"):
    add(json.loads(line))

# non-ASCII before a syntax error: byte columns
for s in ['{"é":x}', '["😀", x]', '["é" x]', '{"a":"日本"}x', '{"a":\n"é€😀" , }', '["é",\n "ü" x]', 'é', '"é',
          '{"aé": 1, "b":}', '{"é😀€": tru}', '["€€€€€€€€€€€€€€€€", nul]', '{"a":"x"}', '"\u0001é"', '[1, "é", 1.5e]']:
    add(s)

# byte order marks
bom8 = b"\xef\xbb\xbf"
add(bom8 + b'{"a":1}'); add(bom8 + b'{x'); add(bom8 + b'\n{"a" 1}'); add(bom8 + bom8 + b'1'); add(bom8)

# invalid UTF-8 inside string values
for bad in [b"\xc3\x28", b"\xe2\x28\xa1", b"\xe2\x82\x28", b"\xf0\x28\x8c\xbc", b"\xf0\x90\x28\xbc", b"\xf0\x90\x8c\x28",
            b"\x80", b"\xbf", b"\xff", b"\xfe", b"\xf8\x88\x80\x80\x80", b"\xc0\x80", b"\xc1\xbf", b"\xed\xa0\x80", b"\xed\xbf\xbf",
            b"\xf4\x90\x80\x80", b"\xf7\xbf\xbf\xbf", b"\xe0\x80\x80", b"\x01", b"\x1f"]:
    add(b'"' + bad + b'"')
    add(b'["ab' + bad + b'cd", 1]')
    add(b'{"a":"' + bad + b'"}')
# truncated multi-byte at end of input, in a string
for t in [b"\xc3", b"\xe2\x82", b"\xe2", b"\xf0\x9f\x98", b"\xf0\x9f", b"\xf0"]:
    add(b'"' + t)
    add(b'["x' + t)
add(b'"abc'); add(b'["abc'); add(b'{"a":"abc'); add(b'"ab\\'); add(b'"ab\\u00'); add(b'"ab\\u00e')

# invalid UTF-8 in field names (short and long names, every quad position)
for bad in [b"\xc3\x28", b"\xff", b"\x80", b"\xed\xa0\x80", b"\xe2\x82\x28", b"\xf0\x90\x28\xbc", b"\xc3", b"\x01"]:
    for pre in [b"", b"a", b"ab", b"abc", b"abcd", b"abcdefgh", b"abcdefghijk", b"abcdefghijklmnop"]:
        add(b'{"' + pre + bad + b'":1}')
for t in [b"\xc3", b"\xe2\x82", b"\xf0\x9f\x98"]:
    add(b'{"ab' + t)
add(b'{"ab'); add(b'{"abcdefghijklmnopq'); add(b'{"\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9\xc3\xa9":1}'); add(b'{"\xf0\x9f\x98\x80":1}')
add(b'{"a\\u00e9":1}'); add(b'{"a\\ud800":1}'); add(b'{"\\ud83d\\ude00":1}'); add(b'{"a\\q":1}'); add(b'{"a\\\xc3\xa9":1}')
add(b'{"a":1,"\xff":2}'); add(b'{"\xc3\xa9":1,"\xc3\xa9":2}')

# non-ASCII bytes where a token is expected
for v in [b"\xc3\xa9", b"\xff", b"\x80", b"\xe2\x82\xac", b"\xf0\x9f\x98\x80", b"\xc2\xa0", b"\xc3", b"\xc3\x28"]:
    add(v); add(b"[" + v + b"]"); add(b"{" + v + b"}"); add(b'{"a":' + v + b"}"); add(b"[1" + v + b"]"); add(b"1" + v)
    add(b"tru" + v); add(b"t" + v); add(b"nul" + v); add(b"[tr" + v + b"]"); add(b" " + v); add(b'"\\' + v + b'"')
    add(b'"\\u00' + v + b'"'); add(b"-" + v); add(b"1." + v); add(b"1e" + v); add(b'{"a" ' + v + b'}'); add(b'["a" ' + v + b']')
    add(b"[truex" + v + b"]"); add(b"[null" + v + b"]")
add(b"[t\xf0\x9f\x98\x80abc]"); add(b"[t\xe6\x97\xa5\xe6\x9c\xac]"); add(b"[tabc\xc3\xa9def]"); add(b"[ \xc3\xa9abc]")
add(b"\x00"); add(b"\x00\x00"); add(b"1\x00"); add(b"[1,\x00]")

# UTF-16 / UTF-32, with and without BOMs
def u16(s, be): return s.encode("utf-16-be" if be else "utf-16-le")
def u32(s, be): return s.encode("utf-32-be" if be else "utf-32-le")
for be in [True, False]:
    for s in ['{"a":"é😀"}', '{"a" 1}', '{"é":x}', '["😀", x]', '1', '[1]', '\n{"a":\n tru}', '"abc', '{"€":1}']:
        add(u16(s, be)); add((b"\xfe\xff" if be else b"\xff\xfe") + u16(s, be))
        add(u32(s, be)); add((b"\x00\x00\xfe\xff" if be else b"\xff\xfe\x00\x00") + u32(s, be))
    add(u16('["a"]', be) + b"\x00")
    add(u16('["a"]', be)[:-1])
    hi = b"\xd8\x3d" if be else b"\x3d\xd8"; lo = b"\xde\x00" if be else b"\x00\xde"
    q = u16('"', be); x = u16("x", be)
    add(q + hi + x + q); add(q + lo + x + q); add(q + hi + q); add(u16('["', be) + hi); add(q + hi + hi + lo + q)
    add(u16('"', be) + (b"\xff\xfe" if be else b"\xfe\xff") + q)
    add(u16('"a', be) + (b"\xfe\xff" if be else b"\xff\xfe") + q)
    def c32(v): return v.to_bytes(4, "big" if be else "little")
    q4 = c32(0x22)
    add(q4 + c32(0x110000) + q4); add(q4 + c32(0xD800) + q4); add(q4 + c32(0xFFFFFFFF) + q4); add(q4 + c32(0x10FFFF) + q4)
    add(u32('["abc"]', be)[:-1]); add(u32('["abc"]', be)[:-2]); add(u32('["abc"]', be) + b"\x00")
    add(u32('{"a" 1, "b": "', be) + c32(0x110000) + q4)
    add(u32('[1]', be) + c32(0x110000))
    add(u32('[' + '1,' * 2100 + '"', be) + c32(0x110000) + q4)
    add(u32('[1 2' + ' ' * 4200, be) + c32(0x110000))
    add(u16('[' + '"é", ' * 2000 + 'x]', be))
add(b"\x00\x00\xff\xfe" + b"\x00\x00\x00\x31"); add(b"\xfe\xff\x00\x00" + b"\x00\x00\x00\x31")
add(b"\x00\x31\x00\x00"); add(b"\x00\x00\x31\x00"); add(b"\x31\x00"); add(b"\x00\x31"); add(b"\x00\x31\x00")
add(b"\x31\x00\x00\x00\x32\x00\x00"); add(b"\xfe\xff"); add(b"\xff\xfe"); add(b"\xfe\xff\x00"); add(b"\xff\xfe\x00\x00")

out = os.path.join(fixtures, "json-bytes-cases.txt")
with open(out, "w") as f:
    for c in cases:
        f.write(c.hex() + "\n")
print(f"{len(cases)} cases -> {out}", file=sys.stderr)
