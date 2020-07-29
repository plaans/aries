//use crate::classical::heuristics::*;
use crate::classical::state::*;
use crate::classical::{GroundProblem/*,World*/};
use crate::explain::state2::*;
use crate::explain::explain::*;
use std::fmt::Display;

//ajout pour gerer fichier
use std::fs::File;
use std::io::{Write, BufReader, BufRead, Error};

//matrice facilite Dijktstra
use nalgebra::base::*;

pub fn causalite2(etape: i32,plan: &Vec<Op> ,initial_state: &State, ops: &Operators,histo: &Vec<Resume>)->(State,Vec<Resume>,Vec<Resume>)/*etat obtenu,histogramme modifié , support*/{
    //initialisation
    let num=etape as usize;
    let opt=plan.get(num);
    let op = opt.unwrap();
    let res = newresume(*op,etape);
    let mut etat=initial_state.clone();
    /*let mut histo = Vec::new();
    for var in initial_state.literals(){
        let res=defaultresume();
        histo.push(res);
    }*/
    //let mut count =0;
    //liste des variables utilisé dans la précond de op
    let mut vecvar=Vec::new();

    //vecteur qui contiendra les resume ayant un lien avec l'op choisis
    let mut link=Vec::new();

    //etape construction histogramme lié
    /*while count < etape {
        let bob=count as usize;
        let opt = plan.get(bob);
        let opt = opt.unwrap();
        let (e,h)=h_step(&etat,opt,ops,count,histo);
        etat=e;
        histo=h;
        count=count+1;
    }   */
    
    //Sélection des variable utilisé dans les préconditions
    let precond = ops.preconditions(*op);
    let mut count2 = 0;
    for var in etat.literals(){
        for pre in precond{
            if var.var()==pre.var(){
                //print!("{} estetetetete ,",count2);
                vecvar.push(count2);
            }
        }
        count2 = count2+1;
    }
    
    //liaison opérateur grâce à histogramme et précondition opé
    ///////
    //Link pas bon
    //////
    for variableutilise in vecvar{
        let resume = histo.get(variableutilise).clone();
        //let resum=resume.unwrap();
        link.push(*resume.unwrap());
    }
    let h = histo.clone();
    let (e1,h2)=h_step(&etat,op,ops,etape,h);
    //println!("{}",link.get(2).unwrap().numero());
   // println!("{}",h2.get(4).unwrap().numero());
    //compare(&e1,&etat);
    //print!("=>");
    //comparehisto(histo,&h2);
    (e1,h2,link)
}

//creer le fichier dot des liens causaux
pub fn fichierdot2<T,I : Display>(plan : &Vec<Op>,ground: &GroundProblem,symbol: &World<T,I> ){

    //fichier de sortie
    let path = "graphique.dot";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "digraph D {{ \n")
        .expect("Something went wrong writing the file");

    //initialisation
    //let plan2 =plan.clone();
    //let plan3 =plan.clone();
    let mut strcause = String::new();
   
    //boucle faire lien causaux de chaque opé plan
    let mut count = 0;//pour suivre etape
    let mut e = ground.initial_state.clone();
    let mut h:Vec<Resume>=Vec::new();//faire init h
    for var in ground.initial_state.literals(){
        let res=defaultresume();
        h.push(res);
    }
    let mut cause : Vec<Resume>=Vec::new();
    for etape in plan{
            //let plan2 =plan3.clone();
            //faire cause
            let (e1,h2,cause)=causalite2(count,/*&*/plan/*2*/,&e,&ground.operators,&h);
            comparehisto(&h,&h2);
            h=h2;
            e=e1.clone();
            let op=plan.get(count as usize).unwrap();
            let opname=&ground.operators.name(*op); 

            //inscription dans fichier

            for res in cause{
                match res.op(){               
                    None => strcause = " i ".to_string(),
                    Some(Resume)=>strcause = format!("{} etape {}",symbol.table.format(&ground.operators.name(res.op().unwrap())),res.numero()),
                    //_ => (),
                }
                let stri=format!("\"{}\" -> \"{} etape {}\";\n",strcause ,symbol.table.format(opname),count);
                write!(output,"{}" ,stri)
                    .expect("Something went wrong writing the file");
            }
            count=count+1;
    }
}

pub fn fichierdotmat<T,I : Display>(support : &DMatrix<i32>, plan : &Vec<Op>,ground: &GroundProblem,symbol: &World<T,I> ){

    //fichier de sortie
    let path = "graphique.dot";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "digraph D {{ \n")
        .expect("Something went wrong writing the file");

    //initialisation
    //let plan2 =plan.clone();
    //let plan3 =plan.clone();
    let mut strcause = String::new();
    
    //boucle faire lien causaux de chaque opé plan
    //let mut count = 0;//pour suivre etape
    //let e,h,cause;
    let t=plan.len();
    let row = support.nrows();
    let col = support.ncols();
    /*for i in plan{
            //let plan2 =plan3.clone();
            //faire cause
            let e,h,cause=causalite2(count,/*&*/plan2,&ground.initial_state,&ground.operators,h);
            let op=plan3.get(count as usize).unwrap();
            let opname=&ground.operators.name(*op); 

            //inscription dans fichier

            for res in cause{
                match res.op(){               
                    None => strcause = " i ".to_string(),
                    Some(Resume)=>strcause = format!("{} etape {}",symbol.table.format(&ground.operators.name(res.op().unwrap())),res.numero()),
                    //_ => (),
                }
                let stri=format!("\"{}\" -> \"{} etape {}\";\n",strcause ,symbol.table.format(opname),count);
                write!(output,"{}" ,stri)
                    .expect("Something went wrong writing the file");
            }
            count=count+1;
    }*/
    for r in 0..row{
        for c in 0..col{
            if support[(r,c)]==1{
                if r==t{
                    strcause = " Goal ".to_string();
                    if c == t{
                        let stri=format!("\"{}\" -> \" Goal \";\n",strcause );

                    }else if c == t+1{
                        let stri=format!("\"{}\" -> \" i \"\n",strcause );

                    }else{
                        let stri=format!("\"{}\" -> \"{} etape {}\";\n",strcause ,symbol.table.format(&ground.operators.name(*plan.get(c).unwrap())),c);
                        write!(output,"{}" ,stri)
                        .expect("Something went wrong writing the file");
                    }

                }else if r == t+1{
                    strcause = " i ".to_string();
                    if c == t{
                        let stri=format!("\"{}\" -> \" Goal \";\n",strcause );
                        write!(output,"{}" ,stri)
                        .expect("Something went wrong writing the file");

                    }else if c == t+1{
                        let stri=format!("\"{}\" -> \" i \"\n",strcause );
                        write!(output,"{}" ,stri)
                        .expect("Something went wrong writing the file");

                    }else{
                        let stri=format!("\"{}\" -> \"{} etape {}\";\n",strcause , symbol.table.format(&ground.operators.name(*plan.get(c).unwrap())),c);
                        write!(output,"{}" ,stri)
                        .expect("Something went wrong writing the file");
                    }

                }else{
                    strcause = format!("{} etape {}",symbol.table.format(&ground.operators.name(*plan.get(r).unwrap())),r);
                    if c == t{
                        let stri=format!("\"{}\" -> \" Goal \";\n",strcause );
                        write!(output,"{}" ,stri)
                        .expect("Something went wrong writing the file");

                    }else if c == t+1{
                        let stri=format!("\"{}\" -> \" i \"\n",strcause );
                        write!(output,"{}" ,stri)
                        .expect("Something went wrong writing the file");

                    }else{
                        let stri=format!("\"{}\" -> \"{} etape {}\";\n",strcause ,symbol.table.format(&ground.operators.name(*plan.get(c).unwrap())),c);
                        write!(output,"{}" ,stri)
                        .expect("Something went wrong writing the file");
                    }
                }
            }
        }
    }
}

pub fn matricesupport2(plan : &Vec<Op>,ground: &GroundProblem)->DMatrix<i32>
{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    //let mut cause =causalitegoals(plan.clone(),init,ops,goals);
    //let mut matrice=DMatrix::from_diagonal_element(length+1,length+1,0);
    let mut matrice=DMatrix::from_diagonal_element(length+2,length+2,0);
//matrice arc lien causaux goal
   /* for r in &cause{
        if r.numero()>=0{
           matrice[(r.numero() as usize,l2 as usize)]=1;
        }
    }*/

    let mut count=0;
    let mut e = ground.initial_state.clone();
    let mut h:Vec<Resume>=Vec::new();
    for var in ground.initial_state.literals(){
        let res=defaultresume();
        h.push(res);
    }
    let mut cause : Vec<Resume>=Vec::new();
    for i in plan{
        let (e1,h2,cause)=causalite2(count,/*&*/plan/*2*/,&e,&ground.operators,&h);
        //println!("c {}, {},",cause.is_empty(),cause.len());
        for r in &cause{
            //println!("cause {}",r.numero());
            if r.numero()>=0{
               // println!("r.num");
                let r=r.numero() as usize;
                let c=count as usize;
                matrice[(r,c)]=1;
            }//prise en compte de l'état initial, pour le calcul de centralité notamment
            else if r.numero()==(-1){
                let row=length+1 as usize;
                let c=count as usize;
                matrice[(row,c)]=1;
            }
        }
        /*let plan3= plan.clone();
        cause =causalite(count,plan3,init,ops);
        //mise à jour matrice lien causaux
        for r in &cause{
            if r.numero()>=0{
                let r=r.numero() as usize;
                let c=count as usize;
                matrice[(r,c)]=1;
            }//prise en compte de l'état initial, pour le calcul de centralité notamment
            else if r.numero()==(-1){
                let row=length+1 as usize;
                let c=count as usize;
                matrice[(row,c)]=1;
            }
        }*/
        count=count+1;
        h=h2;
        e=e1;
    }
    matrice	
}

pub fn matricemenace2(plan : &Vec<Op>,ground: &GroundProblem)->DMatrix<i32>
{
    let ops=&ground.operators;
    let length=plan.len();
    //let l2=length as u32;
    let plan1=plan.clone();

    let plan3=plan.clone();
    let mut matrice=DMatrix::from_diagonal_element(length+1,length+1,0);
//matrice arc lien causaux goal
    let mut cause : Vec<Vec<Resume>>= Vec::new();
    let mut step = 0 as i32;
    for i in plan1{
        let plan2=plan.clone();
        //let e = causalite(step,plan2,&ground.initial_state,ops);
        cause.push(e);
        step=step+1;
    }
    let plan2=plan.clone();
    let mut count = 0;
    for i in plan2{
        let mut count2=0;
        for j in &plan3 {
            if count!=count2{
                //Vec de resume
                let support=cause.get(count);
                let support2=cause.get(count2);
                let mut supportbool = true;
                for su in support{
                    for s in su{
                        if s.op().is_none()==false{
                            if *j == s.op().unwrap(){
                                supportbool=false;
                            }
                        }
                    }
                }
                for su in support2{
                    for s in su{
                        if s.op().is_none()==false{
                            if i == s.op().unwrap(){
                                supportbool=false;
                            }
                        }
                    }
                }
                if supportbool{
                    let precon = ops.preconditions(i);
                    let effet = ops.effects(*j);
                    for pre in precon{
                        for eff in effet{
                            if pre.var() == eff.var(){
                                if count2>count{
                                    matrice[(count2,count)]=1;//1 on place i après j
                                }else{
                                    for su in support{
                                        for s in su{
                                            if !s.op().is_none(){
                                                let effs = ops.effects(s.op().unwrap());
                                                for f in effs{
                                                    if eff.var()==f.var(){
                                                        let ot=s.numero() as usize;
                                                        //changt aberration 14 11 block
                                                        if count2<ot{
                                                            matrice[(count2,ot)]=-1;//-1 on place i avant j
                                                            matrice[(count2,count)]=-2;
                                                        }else{
                                                            matrice[(count2,count)]=-1;
                                                        }
                                                        
                                                    }
                                                }
                                            }
                                            
                                            
                                        }
                                    }
                                }
                                
                            }
                        }
                    }
                }
            }
            count2= count2 +1 ;
        }
        count=count+1;
    }
    matrice	
}