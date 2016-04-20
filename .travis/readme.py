#!/usr/bin/env python

import re

f = open('README.md', 'r')
raw = f.read()

for (i, code) in enumerate(re.findall(r'```rust([^`]*)```', raw, re.M)):
    with open('examples/readme_%s.rs' % i, 'w') as f:
        f.write('#![deny(warnings)]%s' % code)
