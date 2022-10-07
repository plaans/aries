



`l  ⇔ a ≤ b`    or  `¬l  ⇔ b < a`

| premice | inference | comment |
|---------|-----------|---------|
|  `l` & `a ≥ 4`  | `b ≥ 4` | fwd |
|  `l` & `b ≤ 3`  | `a ≤ 3` | fwd |
|  `¬l` & `b ≥ 4` | `a ≥ 5` | bwd |
|  `¬l` & `a ≤ 4` | `b ≤ 3` | bwd |
| `b < 5` &`5 ≤ a` | `¬l` | theory prop (bounds) | 
| `a ≤ 5` & `5 ≤ b` | `l` | theory prop (bounds) |



## With optionals

- `v = presence(l) = valid(a & b)`
- pa = presence(a)
- pb = presence(b)


| premice | inference | comment |
|---------|-----------|---------|
|  `v` & `l` & `a ≥ 4`  | `b ≥ 4` | fwd |
| .. `pb ⇒ v` & `l` & `a ≥ 4`  | `b ≥ 4` | .. eager |
|  `v` & `l` & `b ≤ 3`  | `a ≤ 3` | fwd |
|  .. `pa ⇒ v` & `l` & `b ≤ 3`  | `a ≤ 3` | .. eager|
|  `¬l` & `b ≥ 4` | `a ≥ 5` | bwd |
|  `¬l` & `a ≤ 4` | `b ≤ 3` | bwd |
| `v` & `b < 5` & `5 ≤ a` | `¬l` | theory prop (bounds) | 
| .. `b < 5` & `5 ≤ a` | `¬l` | theory prop (bounds) | 
| `a ≤ 5` & `5 ≤ b` | `l` | theory prop (bounds) |