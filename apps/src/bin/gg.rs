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

    let dom = std::fs::read_to_string(domain_file)?;

    let prob = std::fs::read_to_string(problem_file)?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let lifted = from_chronicles(&spec)?;

    let grounded = grounded_problem(&lifted)?;

    let symbols = &lifted.world.table;
    let search_result = plan_search(&grounded.initial_state, &grounded.operators, &grounded.goals, &config);
    let end_time = std::time::Instant::now();
    let runtime = end_time - start_time;
    let result = match search_result {
        Some(plan) => {
            println!("Got plan: {} actions", plan.len());
            println!("=============");
            for &op in &plan {
                println!("{}", symbols.format(grounded.operators.name(op)));
            }
            let start_time2 = std::time::Instant::now();
            println!("=============");
            let planmenace2=plan.clone();
            /*fichierdotmenace2(planmenace2,&grounded,&lifted.world);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======{}=======",runtime2.as_millis());
            let planot=plan.clone();
            fichierdottemp(planot,&grounded,&lifted.world);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======{}=======",runtime2.as_millis());*/

            //matrice
            println!("---------------matrice support----------------");
            let mat = matricesupport2(&plan,&grounded);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            affichagematrice(&mat);
            println!("---------------------------{}--",runtime2.as_millis());//9610
            println!("---------------matrice support Arthur----------------");
            let mat = matricesupport3(&plan,&grounded);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            /*let mat2 = matricesupport(&plan,&grounded);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            affichagematrice(&mat2);
            comparematrice(&mat,&mat2);*/
            println!("---------------matrice menace--------------{}--",runtime2.as_millis());//504416
            
            
            /*let matm = matricemenace(&plan,&grounded);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            affichagematrice(&matm);
            //explication2etape(&plan, &matm, &mat, /*&grounded,*/ 11, 14);*/
            println!("---------------matrice menace-2======={}",runtime2.as_millis());//496079->486179
            let matm2 = matricemenace2(&plan,&grounded);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            affichagematrice(&matm2);
            //comparematrice(&matm,&matm2);
            //explication2etape(&plan, &matm, &mat, /*&grounded,*/ 11, 14);
            println!("============={}",runtime2.as_millis());//506554->10475;

           /* fichierdot2(&plan,&grounded,&lifted.world);//33000->14000
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======{}=======",runtime2.as_millis());*/
            fichierdotmat(&mat,&plan,&grounded,&lifted.world);//300
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======{}=======",runtime2.as_millis());
            /*fichierdottempmat(&mat,&plan,&grounded,&lifted.world);//
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======{}=======",runtime2.as_millis());*/
            fichierdottempmat2(&mat,&matm2,&plan,&grounded,&lifted.world);//
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======{}=======",runtime2.as_millis());
            fichierdotmenacemat(&matm2,&plan,&grounded,&lifted.world);//
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;

/*
            println!("======centra{}=======",runtime2.as_millis());
            let v=calculcentraliteglobal2(&mat);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            /*for i in &v{
                println!{"{:?}",*i}
            }*/
            let d=regroupementcentralite(&v, &plan);
            affichageregroucentra(d,&grounded,&lifted.world);
            println!("=====");
            let v2=calculcentraliteglobal(&mat);
            let h=regroupementcentraliteop(&v2,&plan);
            affichagehmapop(h,&grounded,&lifted.world);

            let Haction=regroupementcentraliteaction(&v2,&plan,&grounded,&symbols);
            affichagehmapaction(Haction, &lifted.world);
*/

            println!("======Question");
            /*let nec= explicabilite(plan.clone(),&grounded);
            let nec = uniexpli(nec);
            for i in nec{
               i.affiche();
            }

            println!("\n======Question 9\n");
*/
            /*let sup = weightwaydetail(7,1,"move".to_string(),&mat,&plan,&grounded,&symbols,2);
            affichageq9d(&sup,&grounded,&symbols);
            let sup = weightwaydetail(7,1,"drop".to_string(),&mat,&plan,&grounded,&symbols,3);
            affichageq9d(&sup,&grounded,&symbols);*/
/*
            let sup = weightwaydetailetape(96,79,73,&mat,&plan,&grounded,&symbols,40);
            affichageq9d(&sup,&grounded,&symbols);
            let sup = weightway(22,4,"fly-airplane".to_string(),&mat,&plan,&grounded,&symbols,40);
            println!("{}",sup);
            let sup = weightwaydetail(22,4,"fly-airplane".to_string(),&mat,&plan,&grounded,&symbols,10);
            affichageq9d(&sup,&grounded,&symbols);
            let sup = weightway(4,22,"load-truck".to_string(),&mat,&plan,&grounded,&symbols,1);
            println!("{}",sup);
            let sup = weightwaydetail(22,4,"load-truck".to_string(),&mat,&plan,&grounded,&symbols,40);
            affichageq9d(&sup,&grounded,&symbols);
            
            let sup = weightwaygoal2(4,"load-truck".to_string(),&mat,&plan,&grounded,&symbols,10);
            affichageq9d(&sup,&grounded,&symbols);

            let sup = weightwaygoal2(79,"load-truck".to_string(),&mat,&plan,&grounded,&symbols,10);
            affichageq9d(&sup,&grounded,&symbols);
            let sup = weightwaydetail(96,79,"load-truck".to_string(),&mat,&plan,&grounded,&symbols,40);
            affichageq9d(&sup,&grounded,&symbols);

           /* println!("======poids======={}",runtime2.as_millis());
            //let predi = choixpredicat(45, &grounded.initial_state);
            //let mut vpredi=Vec::new();
            //vpredi.push(predi);
            let vpredi=choixpredaction(6, &plan,&grounded);
            let poids=dijkstrapoids(&plan, &grounded, &mat, &vpredi,12);
            for i in poids{
                if i.opnec().numero()== 9{
                    i.affiche();
                }
            }
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======poids 1 et 2======={}",runtime2.as_millis());
            //let predi = choixpredicat(45, &grounded.initial_state);
            //let mut vpredi=Vec::new();
            //vpredi.push(predi);
            let vpredi=choixpredaction2(5, &plan,&grounded);
            let poids=dijkstrapoids(&plan, &grounded, &mat, &vpredi,12);
            for i in poids{
                if i.opnec().numero()== 9{
                    i.affiche();
                }
            }
            println!("==");
            let vpredi=choixpredaction(5, &plan,&grounded);
            let poids=dijkstrapoids(&plan, &grounded, &mat, &vpredi,12);
            for i in poids{
                if i.opnec().numero()== 9{
                    i.affiche();
                }
            }
            println!("======poids======={}",runtime2.as_millis());
            //let predi = choixpredicat(45, &grounded.initial_state);
            //let mut vpredi=Vec::new();
            //vpredi.push(predi);

            let vpredi=choixpredaction2(6, &plan,&grounded);
            let poids=dijkstrapoidsavantage(&plan, &grounded, &mat, &vpredi,12);
            for i in poids{
                if i.opnec().numero()== 9{
                    i.affiche();
                }else if i.opnec().numero()== 3{
                    i.affiche();
                }
            }
            let vpredi=choixpredaction(6, &plan,&grounded);
            let poids=dijkstrapoidsavantage(&plan, &grounded, &mat, &vpredi,12);
            for i in poids{
                if i.opnec().numero()== 9{
                    i.affiche();
                }else if i.opnec().numero()== 3{
                    i.affiche();
                }
            }
            println!("======poids 3=======");
            let vpredi=choixpredaction3("move".to_string(), &plan,&grounded, &lifted.world.table);
            let poids=dijkstrapoidsavantage(&plan, &grounded, &mat, &vpredi,12);
            for i in poids{
                if i.opnec().numero()== 9{
                    i.affiche();
                }else if i.opnec().numero()== 3{
                    i.affiche();
                }
            }
            let vpredi=choixpredaction3("pick".to_string(), &plan,&grounded, &lifted.world.table);
            let poids=dijkstrapoidsavantage(&plan, &grounded, &mat, &vpredi,12);
            for i in poids{
                if i.opnec().numero()== 9{
                    i.affiche();
                }else if i.opnec().numero()== 3{
                    i.affiche();
                }
            }

            //poids.get(60).unwrap().affiche();*/
            
*/
            println!("Essai groupe");
            let groupe= abstractionop(&mat,&plan,&grounded);
            for i in groupe{
                println!("groupe:");
                for op in i{
                    print!("{:?}, {} ",op, symbols.format(grounded.operators.name(op)))
                }
                println!("");
            }

            let vpredi=choixpredaction3("move".to_string(), &plan,&grounded,&symbols);

            println!("Essai groupe action");
            let groupe= abstractionaction(&mat,&plan,&grounded,&symbols);
            for i in groupe{
                println!("groupe:");
                //refaire affichage groupe
                for op in i{
                    print!("{:?}, {} ",op, symbols.format(&[op]));
                }
                println!("");
            }
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======Fin en {}=======",runtime2.as_millis());
/*
            let mut param=Vec::new();
            let a ="rooma".to_string();
            param.push(a);
            let a ="roomb".to_string();
            param.push(a);
            let h = coordination(&param, &plan, &grounded, &symbols);
            affichagecoordination(h, &grounded, &lifted.world);
            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;*/
            println!("======Fin parametre en {}=======",runtime2.as_millis());
            //let fw=floydwarshall(&mat);
            //affichagematrice(&fw);
            //let (fw2,next) = floydwarshallpath(&mat);
            //comparematrice(&fw,&fw2);
            //affichagematrice(&next);
            /*let chemin=path(0,10,&next);
            print!("chemin : ");
            for i in chemin{
                print!("{}->",i);
            }*/
            let cb=betweeness(&mat);
            let mut count =0;
            println!("centralité :");
            for i in &cb{
                println!("etape {} de valeur:{}",count,i);
                count = count+1;
            }

            let end_time2 = std::time::Instant::now();
            let runtime2 = end_time2 - start_time2;
            println!("======Fin intermédiarité en {}=======",runtime2.as_millis());
/*
            xdijkstra(plan.clone(),&grounded);
            xmenace2(plan.clone(),&grounded);
*/
            SolverResult {
                status: Status::SUCCESS,
                solution: Some(Solution::SAT),
                cost: Some(plan.len() as f64),
                runtime,
            }
        }
        None => SolverResult {
            status: Status::SUCCESS,
            solution: Some(Solution::UNSAT),
            cost: None,
            runtime,
        },
    };

    println!("{}", result);
    if opt.expect_sat && !result.proved_sat() {
        std::process::exit(1);
    }
    if opt.expect_unsat && result.solution != Some(Solution::UNSAT) {
        std::process::exit(1);
    }
    Ok(())
}

struct SolverResult {
    status: Status,
    solution: Option<Solution>,
    cost: Option<f64>,
    runtime: std::time::Duration,
}
impl SolverResult {
    pub fn proved_sat(&self) -> bool {
        match self.solution {
            Some(Solution::SAT) => true,
            Some(Solution::OPTIMAL) => true,
            _ => false,
        }
    }
}
impl std::fmt::Display for SolverResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[summary] status:{} solution:{} cost:{} runtime:{}ms",
            match self.status {
                Status::SUCCESS => "SUCCESS",
                Status::TIMEOUT => "TIMEOUT",
                Status::CRASH => "CRASH",
            },
            match self.solution {
                Some(Solution::SAT) => "SAT",
                Some(Solution::UNSAT) => "UNSAT",
                Some(Solution::OPTIMAL) => "OPTIMAL",
                None => "_",
            },
            self.cost.map_or_else(|| "_".to_string(), |cost| format!("{}", cost)),
            self.runtime.as_millis()
        )
    }
}

// TODO: either generalize in the crate or drop
//       when doing so, also remove the clippy:allow at the top of this file
enum Status {
    SUCCESS,
    TIMEOUT,
    CRASH,
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
enum Solution {
    UNSAT,
    SAT,
    OPTIMAL,
}
