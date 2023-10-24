#!/usr/bin/env python3
"""A script to clean up the vendor directory before a .deb build.

The deb build tooling strips out .gitignore files, and .cargo-checksum.json
files contain checksums for them that fail the build if the files are
not present. We don't care about those files, so this script just strips
them from the checksum bundles.
"""

import os
import json

def main():
  for root, _, files in os.walk('vendor'):
    for file in files:
      if file != '.cargo-checksum.json':
        continue

      checksum_file = os.path.join(root, file)
      stripped_json = ""
      with open(checksum_file) as jsonf:
        checksums = json.loads(jsonf.read())
        to_del = []
        for file in checksums["files"]:
          if file.endswith(".gitignore"):
            to_del.append(file)
        for file in to_del:
          del checksums["files"][file]
        stripped_json = json.dumps(checksums)
      with open(checksum_file, "w") as jsonf:
        jsonf.write(stripped_json)

if __name__ == "__main__":
  main()
