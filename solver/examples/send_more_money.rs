#![allow(non_snake_case)]

use aries_solver::prelude::*;
use std::collections::HashMap;

// Takes a str with format [A-Z]+'+'[A-Z]+'='[A-Z]+
// Each of the 3 words ([A-Z]+) represents a number, with each distinct letter being a different digit.
// The sum of the 1st and 2nd word must equal the 3rd word.
// The first letter of each word is not zero.
// Returns an option of a HashMap where the keys are the letters and the values the solved values for each letter.
fn solve(problem: &str) -> Option<HashMap<char, usize>> {
    let mut model = Model::new();

    //Transforming the problem into an array of 3 words
    let mut words: [&str; 3] = [""; 3];
    (words[0], words[1]) = problem.split_once('+').unwrap();
    (words[1], words[2]) = words[1].split_once('=').unwrap();
    print_problem(words);

    //create a decision variable par letter and check validity of the words
    let mut vars: HashMap<u8, Var> = HashMap::new();
    for word in words {
        assert!(!word.is_empty());
        let bytes = word.as_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            assert!(byte.is_ascii_uppercase());
            if i == 0 {
                //can replace a variable ranged [0,9]
                vars.insert(byte, model.new_variable(1, 9));
            } else {
                //can't replace a variable ranged [1,9]
                vars.entry(byte).or_insert_with(|| model.new_variable(0, 9));
            }
        }
    }

    // create linear expressions containing the sums
    let sum: LinSum = word_to_linsum(words[0], &vars) + word_to_linsum(words[1], &vars);
    let result: LinSum = word_to_linsum(words[2], &vars);

    //Constraints of the problem
    model.enforce(sum.eq(result));
    let vars_values: Vec<Var> = vars.values().cloned().collect();
    model.enforce(all_different(vars_values));

    let mut solver = Solver::new(model);

    match solver.solve(SearchLimit::None) {
        Ok(Some(solution)) => {
            //converts the solution into readable format
            let vars_solved: HashMap<char, usize> = vars
                .iter()
                .map(|(k, v)| {
                    (
                        *k as char,
                        solution.eval(*v).expect("All letters should have a value.") as usize,
                    )
                })
                .collect();
            print_solution(words, &vars_solved);
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

fn word_to_linsum(word: &str, vars: &HashMap<u8, Var>) -> LinSum {
    let mut sum: LinSum = LinSum::zero();
    let bytes = word.as_bytes();
    for (i, &byte) in bytes.iter().enumerate() {
        sum += vars[&byte] * 10i32.pow((bytes.len() - 1 - i) as u32);
    }
    sum
}

fn padding(len: usize) {
    for _i in 0..len {
        print!(" ");
    }
}
fn print_word(word: &str) {
    let bytes = word.as_bytes();
    for byte in bytes.iter() {
        print!("{}", *byte as char);
    }
}
fn print_word_value(word: &str, vars_solved: &HashMap<char, usize>) {
    let bytes = word.as_bytes();
    for byte in bytes.iter() {
        print!("{}", vars_solved.get(&(*byte as char)).unwrap());
    }
}

fn print_problem(words: [&str; 3]) {
    println!("Problem:");
    let max_len = words.iter().map(|w| w.len()).max().unwrap();
    padding(max_len - words[0].len() + 2);
    print_word(words[0]);
    print!("\n+ ");
    padding(max_len - words[1].len());
    print_word(words[1]);
    print!("\n= ");
    padding(max_len - words[2].len());
    print_word(words[2]);
    println!("\n");
}

fn print_solution(words: [&str; 3], vars_solved: &HashMap<char, usize>) {
    println!("Solution:");
    let max_len = words.iter().map(|w| w.len()).max().unwrap();
    padding(max_len - words[0].len() + 2);
    print_word_value(words[0], vars_solved);
    print!("\n+ ");
    padding(max_len - words[1].len());
    print_word_value(words[1], vars_solved);
    print!("\n= ");
    padding(max_len - words[2].len());
    print_word_value(words[2], vars_solved);
    println!("\n");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_send_more_money() {
        let sol: HashMap<char, usize> = HashMap::from([
            ('S', 9 as usize),
            ('E', 5 as usize),
            ('N', 6 as usize),
            ('D', 7 as usize),
            ('M', 1 as usize),
            ('O', 0 as usize),
            ('R', 8 as usize),
            ('Y', 2 as usize),
        ]);
        assert_eq!(solve("SEND+MORE=MONEY").expect("Should have a solution."), sol);
    }

    #[test]
    fn test_symetry() {
        assert_eq!(
            solve("SEND+MORE=MONEY").expect("Should have a solution."),
            solve("MORE+SEND=MONEY").expect("Should have a solution.")
        );
    }

    #[test]
    fn unsolvable_example() {
        assert_eq!(solve("OO+OOO=OO"), None);
    }
}
