#![allow(dead_code)]

use anyhow::*;
use aries_planning::classical::search::{plan_search, Cfg};
use aries_planning::classical::{from_chronicles, grounded_problem};
use aries_planning::parsing::pddl_to_chronicles;
use aries_planning::classical::state::Op;
use aries_planning::explain::cause::*;
use aries_planning::explain::explain::*;
use aries_planning::explain::centralite::*;
use aries_planning::explain::question::*;

use std::fmt::Formatter;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use std::fs::File;
use std::io;/*::{Write, BufReader, BufRead, Error,stdin};*/
use std::io::{Write};


#[derive(Debug, StructOpt)]
#[structopt(name = "explainable")]
struct Opt {
    /// If not set, `explain` will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[structopt(long, short)]
    domain: Option<String>,
    problem: String,
    plan: String,

 /*   #[structopt(short = "w", default_value = "3")]
    h_weight: f32,
    #[structopt(long)]
    no_lookahead: bool,*/

    ///Generate dot file for support
    #[structopt(short = "s",long="support")]
    support:bool,
    
    ///Generate dot file for threat
    #[structopt(short = "m",long="threat")]
    menace: bool,
    
    ///Generate dot file for temporal representation
    #[structopt(short = "t",long="temp")]
    temp: bool,  

    ///Ask question
    #[structopt(short = "q",long="question", default_value = "0" )]
    question : String,

    ///display plan
    #[structopt(short = "p",long="plan" )]
    affiche : bool,

    ///Interactive mode
    #[structopt(short = "i",long="interact")]
    interact: bool,  

}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let start_time = std::time::Instant::now();

    let mut config = Cfg::default();
   // config.h_weight = opt.h_weight;
    //config.use_lookahead = !opt.no_lookahead;

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

    println!("parse the plan");
    //parse fichier plan
    let mut plan = Vec::new();
    let mut lines = plan_string.lines();
    //liste opérateur
    //let mut listop = grounded.operators.iter();
    //trouver op correspondant à chaque lignes
    /*for op in grounded.operators.iter(){
        let mut count=0;
        for c in lines.clone(){
            let a = symbols.format(grounded.operators.name(op));
            if a == c {
                plan.insert(count,op);
                //plan.push(op);
                count = count+1;
            }
        }
    }*/
    for c in lines.clone(){
        for op in grounded.operators.iter(){
            let a = symbols.format(grounded.operators.name(op));
            if a == c {
                plan.push(op);
            }
        }
    }

    println!("research support");

    //Traitement
    let mut mat = matricesupport3(&plan,&grounded);
    let mut matm = matricemenace2(&plan,&grounded);
    //Non interactif
    if affiche {
        println!("Got plan: {} actions", plan.len());
        println!("=============");
        let mut count = 0;
        for &op in &plan {
            println!("{}:{}", count,symbols.format(grounded.operators.name(op)));
            count = count+1;
        }
        println!("");
    }    
    if menace{
        println!("file graphique.dot created for support relations");
        fichierdotmenacemat(&matm,&plan,&grounded,&lifted.world);
    }
    if support{
        println!("file graphiquemenace2.dot created for threat relations");
        fichierdotmat(&mat,&plan,&grounded,&lifted.world);
    }
    if temp{
        println!("file graphiquetemp.dot created");
        fichierdottempmat2(&mat,&matm,&plan,&grounded,&lifted.world);
    }
    //let mut decompoquestion=question.chars();

   let mut decompoquestion = Vec::new();

   if question != "0" {
        for i in question.rsplit(' '){
            //println!("{}",i);
            decompoquestion.insert(0,i);
            
        }
        choixquestionsmultiple(&decompoquestion, &mat, &matm, &plan, &grounded, &lifted.world, &symbols);
   }

   let mut rien=false;
   
   if !support & !menace & !temp & !affiche {
       rien =true;
   }
    //Interactif
    if interact | rien {
        let  mut bool = true;
        while bool {
            //affichagematrice(&mat);
            println!("What do you want to do?");
            let mut guess = String::new();

            io::stdin()
                .read_line(&mut guess)
                .expect("Failed to read line");
            
            let mut decompo = Vec::new();
            println!("-----Response------ \n");
            for index in guess.split_whitespace(){
                //println!("{}",i);
                //decompo.insert(0,index);
                decompo.push(index);
            }
            /*for g in &decompo{
                println!("{}",*g);
            }*/
        
            let mut cmd=decompo[0];
            //println!("-{}-",cmd);

            match cmd {
                "s" | "support"=>{ fichierdotmat(&mat,&plan,&grounded,&lifted.world);println!("File graphique.dot rewrited for support relations  ");affichagematrice(&mat); },
                "m" | "threat"=>{ fichierdotmenacemat(&matm,&plan,&grounded,&lifted.world);println!("file graphiquemenace2.dot rewrited for threat relations");affichagematrice(&matm); },
                "q" | "question"=>{
                    //let q=decompo[1];
                    decompo.remove(0);
                    //choixquestions(&decompo, &mat, &matm, &plan, &grounded, &lifted.world, &symbols);
                    choixquestionsmultiple(&decompo,  &mat, &matm, &plan, &grounded, &lifted.world, &symbols)
                    /*match q {
                        "0"=> println!(""),
                        "1"=> {
                            let mystring = decompo[2].to_string();
                            let num = mystring.parse::<usize>().unwrap();
                            let v = supportedby(num,&mat,&plan);
                            affichageq1(num,&plan,v,&grounded,&lifted.world);
                            println!("");
                        },
                        "2"=>  {
                            let mystring = decompo[2].to_string();
                            let num = mystring.parse::<usize>().unwrap();
                            let v = supportof(num,&mat,&plan);
                            affichageq2(num,&plan,v,&grounded,&lifted.world);
                            println!("");
                        },
                        "3"=> {
                            let mystring1 = decompo[2].to_string();
                            let num1 = mystring1.parse::<usize>().unwrap();
                            let mystring2 = decompo[3].to_string();
                            let num2 = mystring2.parse::<usize>().unwrap();
                            let v = menacefromto(num1,num2,&matm);
                            affichageq3(num1,num2,v,&plan,&grounded,&lifted.world);
                            println!("");
                        },
                        "4"=> {
                            let mystring = decompo[2].to_string();
                            let num = mystring.parse::<usize>().unwrap();
                            let v = isnecessary(num,&mat,&plan,&grounded);
                            affichageq4(num,v,&plan,&grounded,&lifted.world);
                            println!("");
                        },
                        "4d"=> {
                            let mystring = decompo[2].to_string();
                            let num = mystring.parse::<usize>().unwrap();
                            let v = isnecessarydetail(num,&mat,&plan,&grounded);
                            affichageqd4(num,v,&plan,&grounded,&lifted.world);
                            println!("");
                        },
                        "5"=>{
                            let mystring1 = decompo[2].to_string();
                            let num1 = mystring1.parse::<usize>().unwrap();
                            let mystring2 = decompo[3].to_string();
                            let num2 = mystring2.parse::<usize>().unwrap();
                            let v = waybetweenbool(num1,num2,&mat,&plan);
                            affichageq5(num1,num2,v,&plan,&grounded,&lifted.world);
                            println!("");
                        } ,
                        "5d"=>{
                            let mystring1 = decompo[2].to_string();
                            let num1 = mystring1.parse::<usize>().unwrap();
                            let mystring2 = decompo[3].to_string();
                            let num2 = mystring2.parse::<usize>().unwrap();
                            let v = waybetween(num1,num2,&mat,&plan);
                            affichageqd5(&v,&grounded,&lifted.world);
                            println!("");
                        } ,
                        "6"=> {
                            let mystring1 = decompo[2].to_string();
                            let num1 = mystring1.parse::<usize>().unwrap();
                            let mystring2 = decompo[3].to_string();
                            let num2 = mystring2.parse::<usize>().unwrap();
                            let v = parallelisablebool(num1,num2,&mat,&matm,&plan,&grounded);
                            affichageq6(v);
                            println!("");
                        },
                        "6d"=> {
                            let mystring1 = decompo[2].to_string();
                            let num1 = mystring1.parse::<usize>().unwrap();
                            let mystring2 = decompo[3].to_string();
                            let num2 = mystring2.parse::<usize>().unwrap();
                            let v = parallelisable(num1,num2,&mat,&matm,&plan,&grounded);
                            affichageqd6(v);
                            println!("");
                        },
                        "7"=> {
                            let mystring = decompo[2].to_string();
                            let num = mystring.parse::<usize>().unwrap();
                            let v = achievegoal(num,&mat);
                            affichageq7(num,v,&plan,&grounded,&lifted.world);
                            println!("");
                        },
                        "8s" => {
                            let t =decompo.len();
                            let mut listparam=Vec::new();
                            for i in 2..t{
                                listparam.push(decompo[i].to_string());
                            }
                            let listesynchro=researchsynchro(&listparam, &mat, &plan, &grounded, &symbols);
                            affichageq8s(&listesynchro, &grounded, &lifted.world);
                            println!("");
                        },
                        "8b"=> {
                            let mystring = decompo[2].to_string();
                            let num = mystring.parse::<usize>().unwrap();
                            let v = nbetweeness(num,&mat,&plan);
                            affichageq8b(v,&grounded,&lifted.world);
                            println!("");

                        },
                        "9"=> unimplemented!(),
                        _=>println!("Not a question available"),
                
                    }*/

                },
                "gg" => {
                    let search_result = plan_search(&grounded.initial_state, &grounded.operators, &grounded.goals, &config);
                    let result = match search_result {
                        Some(plan2) => {
                            println!("Got plan: {} actions", plan2.len());
                            println!("=============");

                            let path = "../plan";        
                            let mut output = File::create(path)
                                .expect("Something went wrong reading the file");

                            for &op in &plan2 {
                                write!(output, "{}\n",symbols.format(grounded.operators.name(op)))
                                        .expect("Something went wrong writing the file");
                                println!("{}", symbols.format(grounded.operators.name(op)));
                            }
                            mat = matricesupport2(&plan2,&grounded);
                            matm = matricemenace2(&plan2,&grounded);
                            plan=plan2;
                        }
                        None => {println!("Got plan");},
                    };
                    
                },
                "p" | "plan" => {
                    println!("Got plan: {} actions", plan.len());
                    println!("=============");
                    let mut count = 0;
                    for &op in &plan {
                        println!("{}:{}", count,symbols.format(grounded.operators.name(op)));
                        count = count+1;
                    }
                    println!("");
                },
                "h" | "help" => {
                    println!("
                    s   Generate dot support and display matrixsupport
                    m   Generate dot threat and display matrix menace
                    q   Question 
                    gg  Make plan with aries planificator if you have suspicion about your plan
                    p   Display plan
                    h   Help
                    e   exit

                    Questions available:
                    -support <step>                             #Display others steps support by step 
                    -supported <step>                           #Display others steps support of step
                    -goal <step>                                #Display true if step accomplish a goal
                    -necessary <step>                           #Display if step participates to the accomplishment of a goal, necessary-d to have the shortest path
                    -path <source-step> <target-step>           #Display path between two steps, path-d to have the path.
                    -threat <source-step> <target-step>         #Display if source step threat target-step if it put right before.
                    -betweeness <n-score>                       #Display all step with a betweeness upper than the n-th score.
                    -synchro <parameters>                       #Display step that make link between group based on parameters
                    -parallelizable <step> <step>               #Display a boolean to know if the two steps are parallelizable, parallelizable-d to have more detail");
                },
                "e" | "exit" => bool=false,
                _=>println!("Not an available entry {}",cmd),

            }
            println!("\n=====End of the interaction=======");
            
        }
        println!("");
    }
    println!("End of the command");
    Ok(())
}