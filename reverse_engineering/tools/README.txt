Tools for the 00CD data files (extracted from bin.psarc dc1/*.bin). Run with: PYTHONPATH=. python -P script.py
fnv.py     FNV-1a 64 (confirmed vs remap.txt)
mkdict.py  builds dict.pkl: hash -> string from tlou-ii.exe strings, archive listings, paks*.txt, index.csv
dump.py    header + top-level table + resolved names of one file   (dump.py weapon-damages.bin)
dc.py      recursive 'map' printer (type hash sits in the 8 bytes BEFORE an object; map = count,keys*,values*)
ann.py     annotated qword/float dump of a byte range
Paths inside are hard-coded to this machine's folders; edit B/D at the top if you move things.
