#![allow(non_snake_case)]

use aries_solver::prelude::*;
use std::collections::HashMap;

/* Takes a str with format [A-Z]+'+'[A-Z]+'='[A-Z]+
Each of the 3 words ([A-Z]+) represents a number, with each distinct letter being a different digit.
The sum of the 1st and 2nd word must equal the 3rd word.
The first letter of each word is not zero.
Returns an option of a Vec with each letter from the problem and its value. Or None if no solution was found.
 */

fn solve(problem: &str) -> Option<Vec<(char, usize)>> {
    let mut model = Model::new();

    //decision variables
    let mut vars: HashMap<u8, Var> = HashMap::new();

    let bytes = problem.as_bytes();
    assert_eq!(problem.len(), bytes.len()); //Check if all characters are ASCII (1 byte)
    let mut pos_plus: usize = 0; //index of '+'
    let mut pos_equal: usize = 0; //index of '='

    for (i, &byte) in bytes.iter().enumerate() {
        if byte == b'+' {
            pos_plus = i;
        } else if byte == b'=' {
            pos_equal = i;
        } else {
            //Other than + and =, the characters must be uppercase letters
            assert!(byte.is_ascii_uppercase());
            if i == 0 || (i == pos_plus + 1 || i == pos_equal + 1) && i != 1 {
                //can replace a variable ranged [0,9]
                vars.insert(byte, model.new_variable(1, 9));
            } else {
                //can't replace a variable ranged [1,9]
                vars.entry(byte).or_insert_with(|| model.new_variable(0, 9));
            }
        }
    }
    //checks if the position of the '+' and '=' satisfy [A-Z]+'+'[A-Z]+'='[A-Z]+
    assert!(0 < pos_plus && pos_plus + 1 < pos_equal && pos_equal < bytes.len() - 2);

    // create linear expressions containing the sums
    let mut sum: LinSum = LinSum::zero();
    let mut result: LinSum = LinSum::zero();
    for i in 0..pos_plus {
        sum += vars[&bytes[i]] * 10i32.pow((pos_plus - 1 - i) as u32);
    }
    for i in pos_plus + 1..pos_equal {
        sum += vars[&bytes[i]] * 10i32.pow((pos_equal - 1 - i) as u32);
    }
    for i in pos_equal + 1..problem.len() {
        result += vars[&bytes[i]] * 10i32.pow((problem.len() - 1 - i) as u32);
    }

    print_problem(pos_plus, pos_equal - pos_plus - 1, bytes.len() - pos_equal - 1, bytes);

    //Constraints of the problem
    model.enforce(sum.eq(result));
    let vars_values: Vec<Var> = vars.values().cloned().collect();
    model.enforce(all_different(vars_values));

    let mut solver = Solver::new(model);

    match solver.solve(SearchLimit::None) {
        Ok(Some(solution)) => {
            //converts the solution into readable format
            let mut vars_solved: Vec<(char, usize)> = Vec::new();
            for &byte in bytes.iter() {
                if byte != b'+' && byte != b'=' {
                    vars_solved.push((
                        byte as char,
                        solution.eval(vars[&byte]).expect("All letters should have a value.") as usize,
                    ));
                }
            }

            print_solution(
                pos_plus,
                pos_equal - pos_plus - 1,
                bytes.len() - pos_equal - 1,
                &vars_solved,
            );

            Some(vars_solved)
        }
        Ok(None) => {
            println!("No solution.\n");
            None
        }
        Err(_) => {
            unreachable!("Solver should not exit without a solution when no search limit is set");
        }
    }
}

fn main() {
    solve("SEND+MORE=MONEY");
}

fn print_problem(len1: usize, len2: usize, len3: usize, bytes: &[u8]) {
    println!("Problem:");
    let max_len = len1.max(len2.max(len3));
    for _i in 0..max_len - len1 + 2 {
        //1st line padding
        print!(" ");
    }
    for (i, byte) in bytes.iter().enumerate() {
        if i == len1 {
            println!();
            print!("+ ");
            for _i in 0..max_len - len2 {
                //2nd line padding
                print!(" ");
            }
        } else if i == len1 + len2 + 1 {
            println!();
            print!("= ");
            for _i in 0..max_len - len3 {
                //3rd line padding
                print!(" ");
            }
        } else {
            print!("{}", *byte as char);
        }
    }
    println!();
    println!();
}

fn print_solution(len1: usize, len2: usize, len3: usize, vars_solved: &[(char, usize)]) {
    println!("Solution:");
    let max_len = len1.max(len2.max(len3));

    for (i, value) in vars_solved.iter().enumerate() {
        if i == 0 {
            for _i in 0..max_len - len1 + 2 {
                //1st line padding
                print!(" ");
            }
        } else if i == len1 {
            println!();
            print!("+ ");
            for _i in 0..max_len - len2 {
                //2nd line padding
                print!(" ");
            }
        } else if i == len1 + len2 {
            println!();
            print!("= ");
            for _i in 0..max_len - len3 {
                //3rd line padding
                print!(" ");
            }
        }
        print!("{}", value.1);
    }
    println!();
    println!();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_send_more_money() {
        let sol: Vec<(char, usize)> = vec![
            ('S', 9),
            ('E', 5),
            ('N', 6),
            ('D', 7),
            ('M', 1),
            ('O', 0),
            ('R', 8),
            ('E', 5),
            ('M', 1),
            ('O', 0),
            ('N', 6),
            ('E', 5),
            ('Y', 2),
        ];
        assert_eq!(solve("SEND+MORE=MONEY").expect("Should have a solution."), sol);
    }

    #[test]
    fn unsolvable_example() {
        assert_eq!(solve("OO+OOO=OO"), None);
    }
}
