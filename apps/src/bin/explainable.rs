#![allow(dead_code)]

use anyhow::*;
use aries_planning::classical::search::{plan_search, Cfg};
use aries_planning::classical::{from_chronicles, grounded_problem};
use aries_planning::parsing::pddl_to_chronicles;
use aries_planning::explain::cause::*;
use aries_planning::explain::explain::*;
use aries_planning::explain::centralite::*;
use aries_planning::explain::question::*;

use std::fmt::Formatter;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use std::fs::File;
use std::io::{Write, BufReader, BufRead, Error};


#[derive(Debug, StructOpt)]
#[structopt(name = "explainable")]
struct Opt {
    /// If not set, `explain` will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[structopt(long, short)]
    domain: Option<String>,
    problem: String,
    plan: String,

    #[structopt(short = "w", default_value = "3")]
    h_weight: f32,
    #[structopt(long)]
    no_lookahead: bool,

    ///Dot file for support
    #[structopt(short = "s")]
    support:bool,
    
    ///Dot file for graphe menace
    #[structopt(short = "m")]
    menace: bool,
    
    ///Dot file for temporal representation
    #[structopt(short = "t")]
    temp: bool,  

    ///Ask question
    #[structopt(short = "q", default_value = "0" )]
    question : String,

    ///afficher plan
    #[structopt(short = "p" )]
    affiche : bool,

    ///exit
    #[structopt(short = "e")]
    exit : bool,

    ///Interactive mode
    #[structopt(short = "i")]
    interact: bool,  

}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let start_time = std::time::Instant::now();

    let mut config = Cfg::default();
    config.h_weight = opt.h_weight;
    config.use_lookahead = !opt.no_lookahead;

    let problem_file = Path::new(&opt.problem);
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();

    let plan_file = Path::new(&opt.plan);
    ensure!(
        plan_file.exists(),
        "plan file {} does not exist",
        plan_file.display()
    );

    let plan_file = plan_file.canonicalize().unwrap();

    let domain_file = match opt.domain {
        Some(name) => PathBuf::from(&name),
        None => {
            let dir = problem_file.parent().unwrap();
            let candidate1 = dir.join("domain.pddl");
            let candidate2 = dir.parent().unwrap().join("domain.pddl");
            if candidate1.exists() {
                candidate1
            } else if candidate2.exists() {
                candidate2
            } else {
                bail!("Could not find find a corresponding 'domain.pddl' file in same or parent directory as the problem file.\
                 Consider adding it explicitly with the -d/--domain option");
            }
        }
    };

    //Récupération des options
    let menace = opt.menace;
    let support = opt.support;
    let temp = opt.temp;
    let question = opt.question;
    let interact = opt.interact;
    let affiche= opt.affiche;
    
    //transformation de pddl
    let dom = std::fs::read_to_string(domain_file)?;

    let prob = std::fs::read_to_string(problem_file)?;

    let plan_string = std::fs::read_to_string(plan_file)?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let lifted = from_chronicles(&spec)?;

    let grounded = grounded_problem(&lifted)?;

    let symbols = &lifted.world.table;

    //test option
    //if menace { println!("menace");}

    //parse fichier plan
    let mut plan = Vec::new();
    let mut lines = plan_string.lines();
    //liste opérateur
    //let mut listop = grounded.operators.iter();
    //trouver op correspondant à chaque lignes
    for c in lines{
        for op in grounded.operators.iter() {
            let a = symbols.format(grounded.operators.name(op));
            if a == c {
                plan.push(op);
            }
        }
    }


    //Traitement
    let mat = matricesupport2(&plan,&grounded);
    let matm = matricemenace2(&plan,&grounded);
    //Non interactif
    if affiche {
        println!("Got plan: {} actions", plan.len());
        println!("=============");

        for &op in &plan {
            println!("{}", symbols.format(grounded.operators.name(op)));
        }
        println!("");
    }    
    if menace{
        fichierdottempmat2(&mat,&matm,&plan,&grounded,&lifted.world);
    }
    if support{
        fichierdotmat(&mat,&plan,&grounded,&lifted.world);
    }
    if temp{
        fichierdottempmat2(&mat,&matm,&plan,&grounded,&lifted.world);
    }
    //let mut decompoquestion=question.chars();

   let mut decompoquestion = Vec::new();

    for i in question.rsplit(' '){
        //println!("{}",i);
        decompoquestion.insert(0,i);
    }

    let q=decompoquestion[0];
    match q {
        "0"=> println!(""),
        "1"=> {
            let mystring = decompoquestion[1].to_string();
            let num = mystring.parse::<usize>().unwrap();
            let v = supportedby(num,&mat,&plan);
            affichageq1(num,&plan,v,&grounded,&lifted.world);
            println!("");
        },
        "2"=>  {
            let mystring = decompoquestion[1].to_string();
            let num = mystring.parse::<usize>().unwrap();
            let v = supportof(num,&mat,&plan);
            affichageq2(num,&plan,v,&grounded,&lifted.world);
            println!("");
        },
        "3"=> {
            let mystring1 = decompoquestion[1].to_string();
            let num1 = mystring1.parse::<usize>().unwrap();
            let mystring2 = decompoquestion[2].to_string();
            let num2 = mystring2.parse::<usize>().unwrap();
            let v = menacefromto(num1,num2,&matm);
            affichageq3(num1,num2,v,&plan,&grounded,&lifted.world);
            println!("");
        },
        "4"=> {
            let mystring = decompoquestion[1].to_string();
            let num = mystring.parse::<usize>().unwrap();
            let v = isnecessary(num,&mat,&plan,&grounded);
            affichageq4(num,v,&plan,&grounded,&lifted.world);
            println!("");
        },
        "4d"=> {
            let mystring = decompoquestion[1].to_string();
            let num = mystring.parse::<usize>().unwrap();
            let v = isnecessarydetail(num,&mat,&plan,&grounded);
            affichageqd4(num,v,&plan,&grounded,&lifted.world);
            println!("");
        },
        "5"=>{
            let mystring1 = decompoquestion[1].to_string();
            let num1 = mystring1.parse::<usize>().unwrap();
            let mystring2 = decompoquestion[2].to_string();
            let num2 = mystring2.parse::<usize>().unwrap();
            let v = waybetweenbool(num1,num2,&mat,&plan);
            affichageq5(num1,num2,v,&plan,&grounded,&lifted.world);
            println!("");
        } ,
        "5d"=>{
            let mystring1 = decompoquestion[1].to_string();
            let num1 = mystring1.parse::<usize>().unwrap();
            let mystring2 = decompoquestion[2].to_string();
            let num2 = mystring2.parse::<usize>().unwrap();
            let v = waybetween(num1,num2,&mat,&plan);
            affichageqd5(&v,&grounded,&lifted.world);
            println!("");
        } ,
        "6"=> {
            let mystring1 = decompoquestion[1].to_string();
            let num1 = mystring1.parse::<usize>().unwrap();
            let mystring2 = decompoquestion[2].to_string();
            let num2 = mystring2.parse::<usize>().unwrap();
            let v = parallelisablebool(num1,num2,&mat,&matm,&plan,&grounded);
            affichageq6(v);
            println!("");
        },
        "6d"=> {
            let mystring1 = decompoquestion[1].to_string();
            let num1 = mystring1.parse::<usize>().unwrap();
            let mystring2 = decompoquestion[2].to_string();
            let num2 = mystring2.parse::<usize>().unwrap();
            let v = parallelisable(num1,num2,&mat,&matm,&plan,&grounded);
            affichageqd6(v);
            println!("");
        },
        "7"=> unimplemented!(),
        "8"=> unimplemented!(),
        "9"=> unimplemented!(),
        _=>println!("Not a question available"),

    }
    //Interactif
    if interact {

    }
    Ok(())
}