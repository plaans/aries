Very simple solver for the knapsack problem.

### Usage

Given items from the following list:

| Name | Weight | Value |
|:----:|:------:|:-----:|
|  a   |   3    |   3   |
|  b   |   3    |   2   |
|  c   |   2    |   2   |

If we want to select a set of items such as the total weight is below 5 that maximizes the value,
we can specify this with the following string, to be passed as the sole argument:

```shell
cargo run --release -- "cap 5 ; a 3 3 ; b 3 2 ; c 2 2"
```
