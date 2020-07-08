use aries::planning::classical::search::*;
use aries::planning::classical::{from_chronicles, grounded_problem};
use aries::planning::parsing::pddl_to_chronicles;

//ajout pour initialisation de l'historique
use aries::planning::classical::state::*;

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

    let prob = std::fs::read_to_string(pb_file).map_err(|o| format!("{}", o))?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let lifted = from_chronicles(&spec)?;

    let grounded = grounded_problem(&lifted)?;

    let symbols = &lifted.world.table;

    match plan_search(
        &grounded.initial_state,
        &grounded.operators,
        &grounded.goals,
    ) {
        Some(plan) => {
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
            let nec=explicabilite(planex,&grounded);
            let nec1=nec.clone();
            for i in nec {
                i.affiche();
            }/*
            nec.get(1).unwrap().affiche();
            nec.get(2).unwrap().affiche();
            nec.get(10).unwrap().affiche();*/
            let nec2=uniexpli(nec1);
            println!("=============");
            for i in nec2 {
                i.affiche();
            }

            let nec3=dijkstra(planex2,&grounded);
            println!("=====Dijk========");
            for i in nec3 {
                i.affiche();
            }

            let planex2=planot.clone();
            let temp=inversibilite(planot,&grounded);
            affichageot(temp);
            fichierdottemp(plandot,&grounded,&lifted.world);
            fichierdotmenace(planmenace,&grounded,&lifted.world);
            fichierdotmenace2(planmenace2,&grounded,&lifted.world);

            //expli

            let planmenace2=planex2.clone();
            xdijkstra(planex2,&grounded);
            xmenace2(planmenace2,&grounded);
        }
        None => println!("Infeasible"),
    }
    


    Ok(())
}
