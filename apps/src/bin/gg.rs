<<<<<<< HEAD:src/bin/gg.rs
use aries::planning::classical::search::*;
use aries::planning::classical::{from_chronicles, grounded_problem};
use aries::planning::parsing::pddl_to_chronicles;
use aries::planning::classical::explain::*;
///home/bjoblot/Documents/aries-master/src/planning/classical/search.rs
///home/bjoblot/Documents/aries-master/src/planning/classical/explain.rs

//ajout pour initialisation de l'historique
use aries::planning::classical::state::*;
=======
#![allow(dead_code)]

use anyhow::*;
use aries_planning::classical::search::{plan_search, Cfg};
use aries_planning::classical::{from_chronicles, grounded_problem};
use aries_planning::parsing::pddl_to_chronicles;

use std::fmt::Formatter;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "gg", rename_all = "kebab-case")]
struct Opt {
    /// If not set, `gg` will look for a `domain.pddl` file in the directory of the
    /// problem file or in the parent directory.
    #[structopt(long, short)]
    domain: Option<String>,
    problem: String,
    #[structopt(short = "w", default_value = "3")]
    h_weight: f32,
    #[structopt(long)]
    no_lookahead: bool,

    /// Make gg return failure with code 1 if it does not solve the problem
    #[structopt(long)]
    expect_sat: bool,

    /// Make gg return failure with code 1 if it does not prove the problem to be unsat
    #[structopt(long)]
    expect_unsat: bool,
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
>>>>>>> 4ce10fd956d458616b416398800935213f38ab82:apps/src/bin/gg.rs

//ajout pour gerer fichier
use std::fs::File;
use std::io::{Write, BufReader, BufRead, Error};

fn main() -> Result<(), String> {
//fichier de sortie

    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() != 3 {
        return Err("Usage: ./gg <domain> <problem>".to_string());
    }
    let dom_file = &arguments[1];
    let pb_file = &arguments[2];

    let dom = std::fs::read_to_string(dom_file).map_err(|o| format!("{}", o))?;
    println!("1");
    let prob = std::fs::read_to_string(pb_file).map_err(|o| format!("{}", o))?;
    println!("2");
    let spec = pddl_to_chronicles(&dom, &prob)?;
    println!("3");
    let lifted = from_chronicles(&spec)?;
    println!("4");
    let grounded = grounded_problem(&lifted)?;
    println!("5");
    let symbols = &lifted.world.table;

    match plan_search(
        &grounded.initial_state,
        &grounded.operators,
        &grounded.goals,
    ) {
        Some(plan) => {
            println!("6");
            // creation 
            let plan2=plan.clone();
            let planex=plan.clone();
            let planot=plan.clone();
/*
            for sta in grounded.initial_state.literals(){
               println!("init state: {} ",sta.val()); 
            }
            */
            println!("init size : {}", grounded.initial_state.size());
            println!("=============");
            println!("Got plan: {} actions", plan.len());
            println!("=============");

            let mut etat = grounded.initial_state.clone();
            let mut histo = Vec::new();
            let mut affichage=Vec::new();
            let mut count =0;
            let mut index =0;
            while index < etat.size() {
                let init=defaultresume();
                histo.push(init);
                index=index+1;
            }

            for &op in &plan {
                //inserer la création de l'état intermediaire ici
                
                //etat=step(&etat,&op,&grounded.operators);
                let (e,h)=h_step(&etat,&op,&grounded.operators,count,histo);
                etat=e;
                histo=h.clone();
                println!("{}", symbols.format(grounded.operators.name(op)));
                if count ==10{
                    affichage=h.clone();
                }

                compare(&etat,&grounded.initial_state);
                count=count+1;
            }


            println!("=============");
            println!("affichage historique etape 10");
            println!("=============");
            let mut var=0;
            for res in affichage{
                if res.numero()>=0 {
                    let opr=res.op();
                    let opr=opr.unwrap();
                    let affiche = &grounded.operators.name(opr);
                    //terminer affichage afficher operator lié à l'Op opr
                    println!("variable {}, {} dernier opérateur à l'avoir modifié, durant l'étape {}", var,symbols.format(affiche) ,res.numero() );
                    //let pre=grounded.operators.preconditions(opr);
                    //println!(" précond {}",*pre.val());
                }
                var=var+1;
            }


            println!("=============");
            println!("affichage cause opérateur");
            println!("=============");
            let cause=causalite(12,plan2,&grounded.initial_state,&grounded.operators);
            let op=plan.get(12).unwrap();
            let opname=&grounded.operators.name(*op);
            println!("Affichage des Opérateur nécessaire à {} de l'étape {}",symbols.format(opname),12);
            println!("=========");
            for res in cause{
                match res.op(){
                    None => println!("variable non changé depuis l'état initial"),
                    Some(Resume)=>println!("{}, de l'étape {}",symbols.format(&grounded.operators.name(res.op().unwrap())),res.numero()),
                    //_ => (),
                }
                
            }


            println!("=============");
            println!("GOALS");
            let iterbut = grounded.goals.iter();
            for but in iterbut{
               println!("goal state: {} ",but.val()); 
            }
            let plandot=plan.clone();
            let planmenace=plan.clone();
            let planmenace2=plan.clone();
            let planex2=plan.clone();
            fichierdot(plan, &grounded, &lifted.world);
            /*let nec=explicabilite(planex,&grounded);
            let nec1=nec.clone();
            for i in nec {
                i.affiche();
            }
            nec.get(1).unwrap().affiche();
            nec.get(2).unwrap().affiche();
            nec.get(10).unwrap().affiche();
            let nec2=uniexpli(nec1);
            println!("=============");
            for i in nec2 {
                i.affiche();
            }*/

            let nec3=dijkstra(planex2,&grounded);
            println!("=====Dijk========");
            for i in nec3 {
                i.affiche();
            }

            let planex2=planot.clone();
            let temp=inversibilite(planot,&grounded);
            affichageot(temp);
            fichierdottemp(plandot,&grounded,&lifted.world);
            //fichierdotmenace(planmenace,&grounded,&lifted.world);
            fichierdotmenace2(planmenace2,&grounded,&lifted.world);

            //expli

            let planmenace2=planex2.clone();
            let planmat=planex2.clone();
            xdijkstra(planex2,&grounded);
            xmenace2(planmenace2,&grounded);

            //matrice
            println!("---------------matrice support----------------");
            let mat = matricesupport(&planmat,&grounded);
            affichagematrice(&mat);

            println!("---------------matrice menace----------------");
            let matm = matricemenace(&planmat,&grounded);
            affichagematrice(&matm);

            println!("---------------explication----------------");
            let nec2=explicationsupport(&planmat, &mat, &grounded, 4, 1);            
            println!("=============");
            nec2.affiche();
            let nec2=explicationsupport(&planmat, &mat, &grounded, 11, 10);
            println!("=============");
            nec2.affiche();
            let nec2=explicationsupport(&planmat, &mat, &grounded, 3, 4);
            println!("=============");
            nec2.affiche();
            let nec2=explicationsupport(&planmat, &mat, &grounded, 4, 3);
            println!("=============");
            nec2.affiche();

            println!("======explication inter étape=======");
            explication2etape(&planmat, &matm, &mat, &grounded, 6, 2);
            println!("=============");
            explication2etape(&planmat, &matm, &mat, &grounded, 11, 14);
        }
        None => println!("Infeasible"),
    }

    Ok(())
}
