use crate::planning::classical::heuristics::*;
use crate::planning::classical::state::*;
use crate::planning::classical::{GroundProblem};
use std::fmt::Display;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

//ajout pour gerer fichier
use std::fs::File;
use std::io::{Write, BufReader, BufRead, Error};

//matrice facilite Dijktstra
/*extern crate matrix;
use matrix::prelude::*;*/
use nalgebra::base::*;

struct Node {
    s: State,
    plan: Vec<Op>,
    f: Cost,
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        Cost::cmp(&self.f, &other.f).reverse()
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Node {}

const WEIGHT: Cost = 3;

pub fn plan_search(initial_state: &State, ops: &Operators, goals: &[Lit]) -> Option<Vec<Op>> {
    let mut heap = BinaryHeap::new();
    let mut closed = HashSet::new();

    let init = Node {
        s: initial_state.clone(),
        plan: Vec::new(),
        f: 0,
    };
    heap.push(init);

    while let Some(n) = heap.pop() {
        if closed.contains(&n.s) {
            continue;
        }
        closed.insert(n.s.clone());
        let hres = hadd(&n.s, ops);
        for &op in hres.applicable_operators() {
            debug_assert!(n.s.entails_all(ops.preconditions(op)));
            let mut s = n.s.clone();
            s.set_all(ops.effects(op));

            let mut plan = n.plan.clone();
            plan.push(op);

            if s.entails_all(goals) {
                return Some(plan);
            }

            let hres = hadd(&s, ops);
            let f = (plan.len() as Cost) + 3 * hres.conjunction_cost(goals);
            let succ = Node { s, plan, f };
            heap.push(succ);
        }
    }

    None
}

//donne état après utilisation op
pub fn step(initial_state: &State, op: &Op, ops: &Operators)-> State {
    let mut suivant =  initial_state.clone();
    let preco =ops.preconditions(*op);
    let effect = ops.effects(*op);

    //inutile le plan est censé etre bon 
    //mais si jamais ont fais tourné sur un plan en construction
    debug_assert!(initial_state.entails_all(preco));
    //passage des effets de l'action sur l'état pour avoir l'état intermediaire
    suivant.set_all(effect);
    suivant
}

//
pub fn compare(initial_state: &State, inter_state: &State){
    let mut diff = 0;
    for lit in initial_state.literals(){
        for liter in inter_state.literals(){
            if lit.var() == liter.var(){
                if lit.val()!=liter.val(){
                    diff=diff+1;
                }
            }
        }
    }
    println!("il y a {} changments entre les états",diff);
}

//donne état après passage op et garde historique des changements
pub fn h_step(initial_state: &State, op: &Op, ops: &Operators, numstep: i32, histo: Vec<Resume>)-> (State,Vec<Resume>){
    let etat=step(initial_state,op,ops);

    let mut count=0;
    let mut newhisto= Vec::new();

    //parcours des vecteurs etatique
    for lit in initial_state.literals(){
        for liter in etat.literals(){
            if lit.var() == liter.var(){
                if lit.val()!=liter.val(){
                    //création d'un nouveau resume et incorporation à l'historique
                    let resume=newresume(*op,numstep);
                    newhisto.push(resume);
                }else{
                    //rien ne change on reprend l'ancien historique
                    let oldresume=histo.get(count);
                    newhisto.push(*oldresume.unwrap());
                    //j'ai essayé mais ça ne fonctionne pas plus
                    //newhisto.push(Some(oldresume));
                }
                count=count+1;            
            }
        }
    }
    (etat,newhisto)
}




//donne les support de l'action Op de l'étape etape
pub fn causalite(etape: i32,plan: Vec<Op> ,initial_state: &State, ops: &Operators)->Vec<Resume>{
    //initialisation
    let num=etape as usize;
    let op=plan.get(num);
    let op = op.unwrap();
    let mut etat=initial_state.clone();
    let mut histo = Vec::new();
    for var in initial_state.literals(){
        let res=defaultresume();
        histo.push(res);
    }
    let mut count =0;
    //liste des variables utilisé dans la précond de op
    let mut vecvar=Vec::new();

    //vecteur qui contiendra les resume ayant un lien avec l'op choisis
    let mut link=Vec::new();

    //etape construction histogramme lié
    while count < etape {
        let bob=count as usize;
        let opt = plan.get(bob);
        let opt = opt.unwrap();
        let (e,h)=h_step(&etat,opt,ops,count,histo);
        etat=e;
        histo=h;
        count=count+1;
    }   
    //Sélection des variable utilisé dans les préconditions
    let precond = ops.preconditions(*op);
    let mut count2 = 0;
    for var in etat.literals(){
        for pre in precond{
            if var.var()==pre.var(){
                vecvar.push(count2);
            }
        }
        count2 = count2+1;
    }

    //liaison opérateur grâce à histogramme et précondition opé
    for variableutilise in vecvar{
        let resume = histo.get(variableutilise).clone();
        //let resum=resume.unwrap();
        link.push(*resume.unwrap());
    }

    link
}

//support des goals
pub fn causalitegoals(plan: Vec<Op> ,initial_state: &State, ops: &Operators, goals: &Vec<Lit>)->Vec<Resume>{
    //initialisation
    let mut etat=initial_state.clone();
    let mut histo = Vec::new();
    for var in initial_state.literals(){
        let res=defaultresume();
        histo.push(res);
    }


    let mut count =0;

    //liste des variables utilisé dans la précond de op
    let mut vecvar=Vec::new();

    //vecteur qui contiendra les resume ayant un lien avec l'op choisis
    let mut link=Vec::new();
    let plan2 =plan.clone();


    //etape construction histogramme lié
    for etape in plan2 {
        let bob=count as usize;
        let opt = plan.get(bob);
        let opt = opt.unwrap();
        let (e,h)=h_step(&etat,opt,ops,count,histo);
        etat=e;
        histo=h;
        count=count+1;
    }
    
    //Sélection des variable utilisé dans les préconditions
    let mut count2 = 0;
    for var in etat.literals(){
        for pre in goals{
            if var.var()==pre.var(){
                vecvar.push(count2);
            }
        }
        count2 = count2+1;
    }

    //liaison opérateur grâce à histogramme et précondition opé
    for variableutilise in vecvar{
        let resume = histo.get(variableutilise).clone();
        //let resum=resume.unwrap();
        link.push(*resume.unwrap());
    }

    link
}

//creer le fichier dot des liens causaux
pub fn fichierdot<T,I : Display>(plan : Vec<Op>,ground: &GroundProblem,symbol: &World<T,I> ){

    //fichier de sortie
    let path = "graphique.dot";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "digraph D {{ \n")
        .expect("Something went wrong writing the file");

    //initialisation
    let plan2 =plan.clone();
    let plan3 =plan.clone();
    //let mut s = String::new();
    let mut strcause = String::new();
   
    //boucle faire lien causaux de chaque opé plan
    let mut count = 0;//pour suivre etape
    for etape in plan{
            let plan2 =plan3.clone();
            //faire cause
            let cause=causalite(count,plan2,&ground.initial_state,&ground.operators);
            let op=plan3.get(count as usize).unwrap();
            let opname=&ground.operators.name(*op);
            //faire string pour 

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
    //pour les goals
    let fin = causalitegoals(plan3,&ground.initial_state,&ground.operators,&ground.goals);
    for res in fin{
        match res.op(){               
                    None => strcause = " i ".to_string(),
                    Some(Resume)=>strcause = format!("{} etape {}",symbol.table.format(&ground.operators.name(res.op().unwrap())),res.numero()),
                }
                let stri=format!("\"{}\" -> goals;\n",strcause);
                write!(output,"{}" ,stri)
                    .expect("Something went wrong writing the file");

    }


    write!(output, "}} ")
       .expect("Something went wrong writing the file");
}

pub fn uniquement(plan : Vec<Op>)->Vec<Unique>{
    let mut un : Vec<Unique>=Vec::new();
    for i in plan{
        let mut count =0;
        for mut g in &mut un{
            if g.operateur() == i{
                g.duplicite();
                count=count+1;
            }
        }
        if count == 0{ un.push(newunique(i));}
    }
    un
}

pub fn explicabilite(plan:Vec<Op>,ground: &GroundProblem )->Vec<Necessaire>{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let plan2=plan.clone();
    let plan4=plan.clone();
    let mut cause =causalitegoals(plan,init,ops,goals);
    let mut out=Vec::new();
    for res in cause{
        out.push(newnecgoal(res));
    }



    //faire support chaque Op
    for a in 1..=length{
        let i=length-a;

        let mut newout=out.clone();
        let u = i as i32;
        println!("etape {}",i);
        let resumei=newresume(*plan2.get(i).unwrap(),u);
        let mut presenceaction= false;
        for nec in &out{
            //si il existe necessaire de l'action i
            if nec.presence(resumei){//list.contains(resumei)
                //besoin de l'action
                let plan3=plan4.clone();
                presenceaction=true;
                cause=causalite(u,plan3,init,ops);
                for res in cause{
                    //verif si les besoins sont deja dans out
                    let mut presence = false;
                    for nec2 in &out{
                        if nec2.presence(res){
                            presence=true;
                            //si chemin present on change qqch si  le chemin est necessaire et plus court
                            if nec2.nec(){
                                if nec.nec(){
                                    //si le nouveau est plus court que l'ancien
                                    if nec2.long()> (nec.long()+1){
                                        let mut newchemin;
                                        if nec.chemin().is_none(){
                                            newchemin=Vec::new();
                                        }else{
                                            newchemin= nec.chemin().unwrap();
                                        }   
                                        newchemin.push(nec.opnec());
                                        newout.push(newnec(res,nec.nec(),newchemin,nec.long()+1));                                    }
                                }

                            }else{
                                //si nouveau chemin necessaire et pas l'ancien
                                if nec.nec(){
                                    let mut newchemin;
                                    if nec.chemin().is_none(){
                                        newchemin=Vec::new();
                                    }else{
                                        newchemin= nec.chemin().unwrap();
                                    }   
                                    newchemin.push(nec.opnec());
                                    newout.push(newnec(res,nec.nec(),newchemin,nec.long()+1));
                                }
                                else{
                                    //si nouveau chemin plus court
                                    if nec2.long()> (nec.long()+1){
                                        let mut newchemin;
                                        if nec.chemin().is_none(){
                                            newchemin=Vec::new();
                                        }else{
                                            newchemin= nec.chemin().unwrap();
                                        }   
                                        newchemin.push(nec.opnec());
                                        newout.push(newnec(res,nec.nec(),newchemin,nec.long()+1));
                                    }
                                }

                            }

                        }
                    }
                    if presence == false{
                        let mut newchemin;
                        if nec.chemin().is_none(){
                            newchemin=Vec::new();
                        }else{
                            newchemin= nec.chemin().unwrap();
                        }   
                        newchemin.push(nec.opnec());
                        newout.push(newnec(res,nec.nec(),newchemin,nec.long()+1));
                    }
                    //si deja verif si chemin plus court
                    //sinon ajouté avec chemin précédent de resumei+resumei
                
                }
            }
        }
        if presenceaction == false {newout.push(newnecess(resumei));}
        out=newout.clone();
    }
    //parcourir support de goal
    //créer necessaire de support
    //remonter le support des necessaires créé
    out
}

//regarde si 2 étapes voisines sont inversibles
pub fn inversibilite(plan: Vec<Op>, ground : &GroundProblem )->Vec<Obligationtemp>{
    let  plan2=plan.clone();
    let plan3=plan.clone();
     let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let taille=plan.len();
    let mut out=Vec::new();
    for u in 1..taille {
        let  plan2=plan.clone();
        let i = u as i32;
        let cause = causalite(i,plan2,init,ops);
        let mut preced=false;
        for res in cause{
            if res.numero() == i-1{
                preced=true;
            }
        }
        if preced == false{
            let precon=plan3.get(u-1);
            let effet = plan3.get(u);
            let precon = ops.preconditions(*precon.unwrap());
            let effet = ops.effects(*effet.unwrap());
            for pre in precon{
                for eff in effet{
                    if pre.var() == eff.var(){
                        let ot=newot(*plan3.get(u-1).unwrap(),i-1,*plan3.get(u).unwrap(),i);
                        out.push(ot);
                    }
                }
            }
        }
        
    }
    out
}

pub fn affichageot(otplan: Vec<Obligationtemp>){
    for i in otplan{
        i.affichage();
    }
}

pub fn uniexpli(neplan: Vec<Necessaire>)->Vec<Necessaire>{
    let mut out : Vec<Necessaire>= Vec::new();


    for i in neplan{
        let  ic = i.clone();
        let mut present = false;
        let mut newout = out.clone();
        let mut count = 0;
        if out.is_empty() == false{ 
            for t in out{
                
                if i.opnec()==t.opnec(){
                    present=true;
                    if i.long()< t.long(){
                        std::mem::replace(&mut newout[count], ic.clone());
                    }
                }
                count = count+1;
            }
        }
        if present == false{
            newout.push(ic);
        }
        out = newout.clone();

    }

    out
}

pub fn fichierdottemp<T,I : Display>(plan : Vec<Op>,ground: &GroundProblem,symbol: &World<T,I> ){

    //fichier de sortie
    let path = "graphiquetemp.dot";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "digraph D {{ \n")
        .expect("Something went wrong writing the file");

    //initialisation
    let plan2 =plan.clone();
    let plan3 =plan.clone();
    //let mut s = String::new();
    let mut strcause = String::new();
   
    //boucle faire lien causaux de chaque opé plan
    let mut count = 0;//pour suivre etape
    for etape in plan{
            let plan2 =plan3.clone();
            //faire cause
            let cause=causalite(count,plan2,&ground.initial_state,&ground.operators);
            let op=plan3.get(count as usize).unwrap();
            let opname=&ground.operators.name(*op);
            //faire string pour 

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
    //pour les goals
     let plan2 =plan3.clone();
    let fin = causalitegoals(plan3,&ground.initial_state,&ground.operators,&ground.goals);
    for res in fin{
        match res.op(){               
                    None => strcause = " i ".to_string(),
                    Some(Resume)=>strcause = format!("{} etape {}",symbol.table.format(&ground.operators.name(res.op().unwrap())),res.numero()),
                }
                let stri=format!("\"{}\" -> goals;\n",strcause);
                write!(output,"{}" ,stri)
                    .expect("Something went wrong writing the file");

    }

    write!(output,"edge [color=red];\n")
        .expect("Something went wrong writing the file");

    let temp=inversibilite(plan2,ground);
    for t in temp{
        let (op1,op2)=t.operateur();
        let (num1,num2)=t.etape();
        let opname1=&ground.operators.name(op1);
        let opname=&ground.operators.name(op2);
        let stri=format!("\"{} etape {}\" -> \"{} etape {}\";\n",symbol.table.format(opname1) ,num1 ,symbol.table.format(opname),num2);
        write!(output,"{}" ,stri)
            .expect("Something went wrong writing the file");

    }


    write!(output, "}} ")
       .expect("Something went wrong writing the file");
}

pub fn menace(plan : Vec<Op>,ground: &GroundProblem)->Vec<Obligationtemp>{
    let plan1=plan.clone();
    let plan2=plan.clone();
    let plan3=plan.clone();
    
    let ops= &ground.operators;

    let mut cause : Vec<Vec<Resume>>= Vec::new();
    let mut out = Vec::new();
    let mut step = 0 as i32;
    for i in plan1{
        let plan4=plan.clone();
        let e = causalite(step,plan4,&ground.initial_state,ops);
        cause.push(e);
        step=step+1;
    }
    let mut count = 0;
    for i in plan2{
        let mut count2=0;
        for j in &plan3 {
            if count!=count2{
                //Vec de resume
                let support=cause.get(count);
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
                if supportbool{
                    let precon = ops.preconditions(*j);
                    let effet = ops.effects(i);
                    for pre in precon{
                        for eff in effet{
                            if pre.var() == eff.var(){
                                let c2=count2 as i32;
                                let c1= count as i32;
                                let ot=newot(*j,c2,i,c1);
                                out.push(ot);
                            }
                        }
                    }
                }
            }
            count2= count2 +1 ;
        }
        count=count+1;
    }
    out
}

pub fn menace2(plan : Vec<Op>,ground: &GroundProblem)->Vec<Obligationtemp>{
    let plan1=plan.clone();
    let plan2=plan.clone();
    let plan3=plan.clone();
    
    let ops= &ground.operators;

    let mut cause : Vec<Vec<Resume>>= Vec::new();
    let mut out = Vec::new();
    let mut step = 0 as i32;
    for i in plan1{
        let plan4=plan.clone();
        let e = causalite(step,plan4,&ground.initial_state,ops);
        cause.push(e);
        step=step+1;
    }
    let mut count = 0;
    for i in plan2{
        let mut count2=0;
        for j in &plan3 {
            if count!=count2{
                //Vec de resume
                let support=cause.get(count);
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
                if supportbool{
                    let precon = ops.preconditions(i);
                    let effet = ops.effects(*j);
                    for pre in precon{
                        for eff in effet{
                            if pre.var() == eff.var(){
                                let c2=count2 as i32;
                                let c1= count as i32;
                                if(c2>c1){
                                    let ot=newot(*j,c2,i,c1);
                                    out.push(ot);
                                }else{
                                    let c2=count2 as i32;
                                    for su in support{
                                        for s in su{
                                            if !s.op().is_none(){
                                                let effs = ops.effects(s.op().unwrap());
                                                for f in effs{
                                                    if eff.var()==f.var(){
                                                        let ot=newot(*j,c2,s.op().unwrap(),s.numero());
                                                        out.push(ot);
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
    out
}


pub fn fichierdotmenace<T,I : Display>(plan : Vec<Op>,ground: &GroundProblem,symbol: &World<T,I> ){

    //fichier de sortie
    let path = "graphiquemenace.dot";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "digraph D {{ \n")
        .expect("Something went wrong writing the file");

    //initialisation
    let plan2 =plan.clone();
    let plan3 =plan.clone();   

    let temp=inversibilite(plan2,ground);
    for t in temp{
        let (op1,op2)=t.operateur();
        let (num1,num2)=t.etape();
        let opname1=&ground.operators.name(op1);
        let opname=&ground.operators.name(op2);
        let stri=format!("\"{} etape {}\" -> \"{} etape {}\";\n",symbol.table.format(opname1) ,num1 ,symbol.table.format(opname),num2);
        write!(output,"{}" ,stri)
            .expect("Something went wrong writing the file");

    }

    let menace=menace(plan3,ground);
    for m in menace{
        let (op1,op2)=m.operateur();
        let (num1,num2)=m.etape();
        let opname1=&ground.operators.name(op1);
        let opname=&ground.operators.name(op2);
        if num1>num2{
            write!(output,"edge [color=red];\n")
                .expect("Something went wrong writing the file");
            let stri=format!("\"{} etape {}\" -> \"{} etape {}\";\n",symbol.table.format(opname1) ,num1 ,symbol.table.format(opname),num2);
            write!(output,"{}" ,stri)
                .expect("Something went wrong writing the file");
        }else{
            write!(output,"edge [color=blue];\n")
                .expect("Something went wrong writing the file");
            let stri=format!("\"{} etape {}\" -> \"{} etape {}\";\n",symbol.table.format(opname1) ,num1 ,symbol.table.format(opname),num2);
            write!(output,"{}" ,stri)
                .expect("Something went wrong writing the file");
        }

    }

    write!(output,"edge [color=red];\n")
        .expect("Something went wrong writing the file");



    write!(output, "}} ")
       .expect("Something went wrong writing the file");
}

pub fn fichierdotmenace2<T,I : Display>(plan : Vec<Op>,ground: &GroundProblem,symbol: &World<T,I> ){

    //fichier de sortie
    let path = "graphiquemenace2.dot";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "digraph D {{ \n")
        .expect("Something went wrong writing the file");

    //initialisation
    let plan2 =plan.clone();
    let plan3 =plan.clone();   

    let temp=inversibilite(plan2,ground);
    for t in temp{
        let (op1,op2)=t.operateur();
        let (num1,num2)=t.etape();
        let opname1=&ground.operators.name(op1);
        let opname=&ground.operators.name(op2);
        let stri=format!("\"{} etape {}\" -> \"{} etape {}\";\n",symbol.table.format(opname1) ,num1 ,symbol.table.format(opname),num2);
        write!(output,"{}" ,stri)
            .expect("Something went wrong writing the file");

    }

    let menace=menace2(plan3,ground);
    for m in menace{
        let (op1,op2)=m.operateur();
        let (num1,num2)=m.etape();
        let opname1=&ground.operators.name(op1);
        let opname=&ground.operators.name(op2);
        if num1>num2{
            write!(output,"edge [color=red];\n")
                .expect("Something went wrong writing the file");
            let stri=format!("\"{} etape {}\" -> \"{} etape {}\";\n",symbol.table.format(opname1) ,num1 ,symbol.table.format(opname),num2);
            write!(output,"{}" ,stri)
                .expect("Something went wrong writing the file");
        }else{
            write!(output,"edge [color=blue];\n")
                .expect("Something went wrong writing the file");
            let stri=format!("\"{} etape {}\" -> \"{} etape {}\";\n",symbol.table.format(opname1) ,num1 ,symbol.table.format(opname),num2);
            write!(output,"{}" ,stri)
                .expect("Something went wrong writing the file");
        }

    }

    write!(output,"edge [color=red];\n")
        .expect("Something went wrong writing the file");



    write!(output, "}} ")
       .expect("Something went wrong writing the file");
}

pub fn dijkstra(plan : Vec<Op>,ground: &GroundProblem)->Vec<Necessaire>{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    let dd=l2+1;
    let plan2=plan.clone();
    let plan3=plan.clone();
    let plan4=plan.clone();
    let mut cause =causalitegoals(plan3,init,ops,goals);
    let plan3=plan.clone();
    //let mut matrix = Conventional::new((length+1, length+1));
    /*let mut mat2 = Matrix::new(dd,dd);
    let mut mat= DMatrix::<u32>::zeros();*/
    let mut matrice=DMatrix::from_diagonal_element(length+1,length+1,0);
    let mut atraite=Vec::new();
    let mut traite=Vec::new();
    /*for i in 0..length+1{
        //matrix.set((i,i),0);
    }*/
    //mat.set_diagonal(0);
//matrice arc lien causaux goal
    for r in &cause{
        if r.numero()>=0{
           // matrix.set((r.numero(),length),1);
           matrice[(r.numero() as usize,l2 as usize)]=1;
        }
    }

    let mut count=0;
    for i in plan2{
        let plan3= plan.clone();
        cause =causalite(count,plan3,init,ops);
        //mise à jour matrice lien causaux
        for r in &cause{
            //println!("init dij");
            if r.numero()>=0{
               // println!("init dij 1");
                //matrix.set((r.numero(),count),1);
                let r=r.numero() as usize;
                let c=count as usize;
                matrice[(r,c)]=1;
                //println!("init dij 1 {}",matrice[(r,c)]);
            }
        }
        count=count+1;
    }

    //dijkstra
/*notation
S la liste des sommets du graphe ;
s0 le sommet du graphe à partir duquel on veut déterminer les plus courts chemins aux autres sommets ;
l(x,y) le poids de l'arête entre deux sommets x et y ;
δs(x) la longueur d'un chemin du sommets s0 au sommet x ;
V+(x) la liste des successeurs du sommet x ;
p(x) le prédécesseur du sommet x ;
X liste des sommets restant à traiter ;
E liste des sommets déjà traités.
*/
    /*init Dij


Pour Chaque x∈S Faire δs(x)←∞  On attribue un poids ∞ à chacun des sommetsx
 δs(s0)←0   Le poids du sommet s0 est nul
 X←S    La liste des sommets restant à traiter est initialisée à S
 E←∅    La liste des sommets déjà traités vide
    */
    cause = causalitegoals(plan3,init,ops,goals);
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2+1);
        //if mene à goal
        for c in &cause{
            if c.numero()==count{
                nec = newnecgoal(step);
            }
        }        
        atraite.push(nec);
        count=count+1;
    }

    /*traitement
Tant queX≠∅Faire            Tant que la liste des sommets restant à traiter n'est pas vide

    Sélectionner dans la liste X le sommet x avec δs(x) minimum
    Retirer le sommet x de la liste X
    Ajouter le sommet x à la liste E
    Pour Chaquey∈V+(x)∩XFaire    On examine tous les successeurs y du sommet x qui ne sont pas traités
        Siδs(y)>δs(x)+l(x,y)Alors
        δs(y)←δs(x)+l(x,y)      La distance du sommet s0 au sommet y est minimale
        p(y)←x             Le sommet x est le prédécesseur du sommet y
        Fin Si
    Fin pour

Fin Tant que
    */
    let mut done= false;
    while !done{
        //sommet chemin plus court
        let mut somme=atraite.get(0).unwrap().clone();
        let mut count = 0;
        let mut index=0;
        for i in &atraite{
            if i.long()<somme.long(){
                somme=i.clone();
                index=count;
            }
            count=count+1;
        }
        //println!{"     retire     {}",index};
        //
        atraite.remove(index);
        let sommec=somme.clone();
        traite.push(sommec);
        //println!("essai entrée dijk");
        //examine tous les successeurs y du sommet x qui ne sont pas traités
        for i in 0..length+1{
            let ind=somme.opnec().numero() as usize;
            //println!("essai entrée dijkstra {} {} {}",matrice[(i,ind)],i,somme.opnec().numero());
            if matrice[(i,ind)]!=0{
                //println!("essai entrée dijk 0");
                let mut newatraite = Vec::new();
                for res in atraite{
                    if res.opnec().numero()==(i as i32){
                        //println!("essai entrée dijk 1");
                        if res.long()>somme.long()+1{
                           /* 
                            
                            let chemi=chem.push(somme.opnec());
                            let chemmi=chemm.push(s);*/
                            //println!("essai entrée dijk 2");
                            //let chem=somme.chemin();
                            //attention unwrap
                            let mut newchemin;
                            if somme.chemin().is_none(){
                                newchemin=Vec::new();
                            }else{
                                newchemin= somme.chemin().unwrap();
                            }   
                            newchemin.push(somme.opnec());
                            let nec=newnec(res.opnec(),somme.nec(),newchemin,somme.long()+1);
                            newatraite.push(nec);
                        }
                        else{newatraite.push(res);}

                    }else{newatraite.push(res);}
                }
                atraite=newatraite.clone();
            }

        }
        if atraite.is_empty(){
            done=true;
        }
    }
    traite
}
/*
pub fn explicationmenace(){

}*/

//
//   EXPLICATION
//

pub fn xdijkstra(plan : Vec<Op>,ground: &GroundProblem)->Vec<Necessaire>{
    let path = "Explisupport.txt";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "Explication des liens de support et de leur Nécessité \n")
        .expect("Something went wrong writing the file");


    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    let dd=l2+1;
    let plan2=plan.clone();
    let plan3=plan.clone();
    let plan4=plan.clone();
    let mut cause =causalitegoals(plan3,init,ops,goals);
    let plan3=plan.clone();
    let mut matrice=DMatrix::from_diagonal_element(length+1,length+1,0);
    let mut atraite=Vec::new();
    let mut traite=Vec::new();

    for r in &cause{
        if r.numero()>=0{
           // matrix.set((r.numero(),length),1);
           matrice[(r.numero() as usize,l2 as usize)]=1;
        }
    }

    let mut count=0;
    for i in plan2{
        let plan3= plan.clone();
        cause =causalite(count,plan3,init,ops);
        //mise à jour matrice lien causaux
        for r in &cause{   
            if r.numero()>=0{
                //matrix.set((r.numero(),count),1);
                let r=r.numero() as usize;
                let c=count as usize;
                matrice[(r,c)]=1;
                write!(output, "L'étape {} est support de l'étape {} \n",r,c)
                    .expect("Something went wrong writing the file");
            }
        }
        count=count+1;
    }

    cause = causalitegoals(plan3,init,ops,goals);
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2+1);
        //if mene à goal
        for c in &cause{
            if c.numero()==count{
                nec = newnecgoal(step);
            }
        }        
        atraite.push(nec);
        count=count+1;
    }
    let mut done= false;
    while !done{
        //sommet chemin plus court
        let mut somme=atraite.get(0).unwrap().clone();
        let mut count = 0;
        let mut index=0;
        for i in &atraite{
            if i.long()<somme.long(){
                somme=i.clone();
                index=count;
            }
            count=count+1;
        }
        //
        atraite.remove(index);
        let sommec=somme.clone();
        traite.push(sommec);
        //examine tous les successeurs y du sommet x qui ne sont pas traités
        for i in 0..length+1{
            let ind=somme.opnec().numero() as usize;
            if matrice[(i,ind)]!=0{
                let mut newatraite = Vec::new();
                for res in atraite{
                    if res.opnec().numero()==(i as i32){
                        if res.long()>somme.long()+1{
                            //attention unwrap
                            let mut newchemin;
                            if somme.chemin().is_none(){
                                newchemin=Vec::new();
                            }else{
                                newchemin= somme.chemin().unwrap();
                            }   
                            newchemin.push(somme.opnec());
                            let nec=newnec(res.opnec(),somme.nec(),newchemin,somme.long()+1);
                            newatraite.push(nec);
                        }
                        else{newatraite.push(res);}

                    }else{newatraite.push(res);}
                }
                atraite=newatraite.clone();
            }

        }
        if atraite.is_empty(){
            done=true;
        }
    }
    for t in &traite{
        if t.nec(){
            if t.chemin().is_none(){
                write!(output, "l'étape {} est nécessaire car elle accomplis un but \n",t.opnec().numero())
                    .expect("Something went wrong writing the file");
            }
            else{
                write!(output, "l'étape {} est nécessaire car elle support au moins un élément qui participe à l'accomplissement d'un but dans le chemin de longueur {} composé par", t.opnec().numero(),t.long())
                    .expect("Something went wrong writing the file");
                for i in t.chemin(){
                    for n in i{
                        write!(output, " de l'étape{}", n.numero())
                        .expect("Something went wrong writing the file");
                    } 
                }
                write!(output, "\n")
                    .expect("Something went wrong writing the file");
            }
            
        }else{
            write!(output, "l'étape {} n'est pas nécessaire \n", t.opnec().numero())
        .expect("Something went wrong writing the file");
        }
    }

    traite
}

pub fn xmenace2(plan : Vec<Op>,ground: &GroundProblem)->Vec<Obligationtemp>{
    let path = "Explimenace.txt";
            
    let mut output = File::create(path)
        .expect("Something went wrong reading the file");

    write!(output, "Explication des menaces entre actions \n")
        .expect("Something went wrong writing the file");


    let plan1=plan.clone();
    let plan2=plan.clone();
    let plan3=plan.clone();
    
    let ops= &ground.operators;

    let mut cause : Vec<Vec<Resume>>= Vec::new();
    let mut out = Vec::new();
    let mut step = 0 as i32;
    for i in plan1{
        let plan4=plan.clone();
        let e = causalite(step,plan4,&ground.initial_state,ops);
        cause.push(e);
        step=step+1;
    }
    let mut count = 0;
    for i in plan2{
        let mut count2=0;
        for j in &plan3 {
            if count!=count2{
                //Vec de resume
                let support=cause.get(count);
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
                if supportbool{
                    let precon = ops.preconditions(i);
                    let effet = ops.effects(*j);
                    for pre in precon{
                        for eff in effet{
                            if pre.var() == eff.var(){
                                let c2=count2 as i32;
                                let c1= count as i32;
                                if(c2>c1){
                                    let ot=newot(*j,c2,i,c1);
                                    write!(output, "L'étape {} est une menace pour l'étape {} et doit être placé après\n",c2,c1)
                                        .expect("Something went wrong writing the file");
                                    out.push(ot);
                                }else{
                                    let c2=count2 as i32;
                                    let c1= count as i32;
                                    for su in support{
                                        for s in su{
                                            if !s.op().is_none(){
                                                let effs = ops.effects(s.op().unwrap());
                                                for f in effs{
                                                    if eff.var()==f.var(){
                                                        let ot=newot(*j,c2,s.op().unwrap(),s.numero());
                                                        write!(output, "L'étape {} et doit être placé avant l'étape {} car elle menace le lien entre {} et {}\n",c2,s.numero(),c1,s.numero())
                                                            .expect("Something went wrong writing the file");
                                                        out.push(ot);
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
    out
}

