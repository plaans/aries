//use crate::classical::heuristics::*;
use crate::classical::state::*;
use crate::classical::{GroundProblem/*,World*/};
use crate::explain::state2::*;
use std::fmt::Display;
use crate::symbols::{SymbolTable,SymId};
use std::collections::HashMap;

//ajout pour gerer fichier
use std::fs::File;
use std::io::{Write, BufReader, BufRead, Error};

//matrice facilite Dijktstra
use nalgebra::base::*;

//ancien search
//------------------------------------------------------------


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
 /*   for v in initial_state.variables()
      if initial_state.value(v) == */
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

pub fn comparehisto(h1:&Vec<Resume>,h2:&Vec<Resume>){
    let mut diff = 0;
    for var1 in h1{
        for var2 in h2{
            if var1.numero() != var2.numero(){
                //println!("{},{}",var1.numero(),var2.numero());
                diff=diff+1;
            }
        }
    }
    println!("il y a {} changments entre les histo",diff);
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
    //let plan2 =plan.clone();
    let plan3 =plan.clone();
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
                                        newout.push(newnec(res,nec.nec(),newchemin,nec.long()+1));
                                    }
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
    let plan3=plan.clone();
     let init=&ground.initial_state;
    let ops=&ground.operators;
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
    let plan3 =plan.clone();
    let mut strcause = String::new();
   
    //boucle faire lien causaux de chaque opé plan
    let mut count = 0;//pour suivre etape
    for etape in plan{
            let plan2 =plan3.clone();
            //faire cause
            let cause=causalite(count,plan2,&ground.initial_state,&ground.operators);
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
                                if c2>c1{
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
    let plan2=plan.clone();
    let plan3=plan.clone();
    let mut cause =causalitegoals(plan3,init,ops,goals);
    let plan3=plan.clone();
    let mut matrice=DMatrix::from_diagonal_element(length+1,length+1,0);
    let mut atraite=Vec::new();
    let mut traite=Vec::new();
//matrice arc lien causaux goal
    for r in &cause{
        if r.numero()>=0{
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
                let r=r.numero() as usize;
                let c=count as usize;
                matrice[(r,c)]=1;
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
    traite
}

pub fn dijkstra2(support: &DMatrix<i32>,plan : Vec<Op>,ground: &GroundProblem)->Vec<Necessaire>{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    let plan3=plan.clone();
    let mut cause =causalitegoals(plan3,init,ops,goals);
    let plan3=plan.clone();
    let mut matrice=support.clone();
    let mut atraite=Vec::new();
    let mut traite=Vec::new();

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
    traite
}

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
    let plan2=plan.clone();
    let plan3=plan.clone();
    let mut cause =causalitegoals(plan3,init,ops,goals);
    let plan3=plan.clone();
    let mut matrice=DMatrix::from_diagonal_element(length+1,length+1,0);
    let mut atraite=Vec::new();
    let mut traite=Vec::new();

    for r in &cause{
        if r.numero()>=0{
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
                                if c2>c1{
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

pub fn matricesupport(plan : &Vec<Op>,ground: &GroundProblem)->DMatrix<i32>
{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    let mut cause =causalitegoals(plan.clone(),init,ops,goals);
    //let mut matrice=DMatrix::from_diagonal_element(length+1,length+1,0);
    let mut matrice=DMatrix::from_diagonal_element(length+2,length+2,0);
//matrice arc lien causaux goal
    for r in &cause{
        if r.numero()>=0{
           matrice[(r.numero() as usize,l2 as usize)]=1;
        }
    }

    let mut count=0;
    for i in plan{
        let plan3= plan.clone();
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
        }
        count=count+1;
    }
    matrice	
}

pub fn matricemenace(plan : &Vec<Op>,ground: &GroundProblem)->DMatrix<i32>
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
        let e = causalite(step,plan2,&ground.initial_state,ops);
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


//------------------------------------------------
//----------------------------------------------





pub fn affichagematrice (matr : &DMatrix<i32>){
    let i = matr.nrows();
    let j = matr.ncols();
    for row in 0..i{
        for col in 0..j{
            let n=matr.get((row,col));
            if !n.is_none(){
                if *n.unwrap()< 0{
                    print!(" {} ", n.unwrap());
                }
                else if *n.unwrap()< 10{
                    print!(" {}  ", n.unwrap());
                }
                else{
                    print!(" {} ", n.unwrap());
                }
                
            }
        }
        println!("");
    }
}


pub fn comparematrice(mat1 : &DMatrix<i32>,mat2 : &DMatrix<i32>){
    let mut diff=0;
    let i1 = mat1.nrows();
    let j1 = mat1.ncols();
    let i2 = mat2.nrows();
    let j2 = mat2.ncols();
    if i1!=i2 || j2!=j1 {
        println!("Matrice de taille différente erreur");
    }
    else{
        for row in 0..i1{
            for col in 0..j1{
                if mat1[(row,col)] != mat2[(row,col)]{
                    diff=diff+1;
                    println!("{},{} de valeur {},{}",row,col,mat1[(row,col)],mat2[(row,col)]);
                }
            }
        }
        println!("Il y a {} différences entre les 2 matrices",diff);
    }
}



//explain
//----------------------------------------------------------







//explication des liens entre 2 points (menaces, support...)

/***
ATTENTION Step 1 est l'étape supporté et step 2 la supportante
*******/
pub fn explicationsupport(plan: &Vec<Op>,support : &DMatrix<i32> , /*ground : &GroundProblem,*/ step1: i32, step2:i32)->Necessaire{
	//dijkstra( plan, ground);
	let length=plan.len();
	let mut atraite=Vec::new();
	let mut traite=Vec::new();
	let l2=length as u32;

//init
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2+1);
        //if mene à step1
		if count == step1 {
			nec = newnecgoal(step);
		}      
        atraite.push(nec);
        count=count+1;
    }

 //Dijk 
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
        atraite.remove(index);
        let sommec=somme.clone();
        traite.push(sommec);
        //examine tous les successeurs y du sommet x qui ne sont pas traités
        for i in 0..length+1{
            let b=i as i32;
            let ind=somme.opnec().numero() as usize;
            if support[(i,ind)]!=0{
                //println!("essai entrée dijk 0");
                let mut newatraite = Vec::new();
                for res in atraite{
					//println!("essai entrée dijk {}, {}",res.opnec().numero(),b);
                    if res.opnec().numero()==b{//ici pb
                        //println!("essai entrée dijk 1");
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
    let s2=step2 as usize;
    let mut step;
    if !(plan.get(s2).is_none()){
        step =newresume(*plan.get(s2).unwrap(),step2);
    }else{
        step=goalresume(step2);
    }
	let mut nec =initnec(step,l2+1);
	for i in traite{
		if i.opnec().numero()==step2{
			nec= i;
		}
	}
	nec
}

pub fn explicationmenace(plan: &Vec<Op>,menace: &DMatrix<i32>,support : &DMatrix<i32> , /*ground : &GroundProblem,*/ step1: i32, step2:i32){
	//dijkstra( plan, ground);
	let length=plan.len();
    let l2=length as u32;
    let s1=step1 as usize;
    let s2=step2 as usize;
    let m=menace.get((s1,s2));
    if !m.is_none(){
        if *m.unwrap() == 0 {
            println!(" L'étape {} n'est pas une menace pour l'étape {}",step1, step2);
        }else if *m.unwrap() == 1{
            println!(" L'étape {} est une menace pour l'étape {} et doit être placer après",step1, step2);
        }else if *m.unwrap() == -1{
            println!(" L'étape {} est une menace pour l'étape {} et doit être placer avant",step1,step2);
        }else if *m.unwrap() == -2{
            //on regarde les support de s2
            let row=support.column(s2);
            for index in 0..l2+1{
                let i = index as usize;
                let support=row.get(i);
                if !support.is_none(){
                    if *support.unwrap()==1 {
                        //on regarde quel support est lié à la menace
                        let ms=menace.get((s1,i));
                        if !ms.is_none() {
                            if *ms.unwrap() == -1{
                                println!(" L'étape {} est une menace pour le lien entre l'étape {} et l'étape {}, l'étape {} doit être placer avant {}",step1, i,step2,step1, i);
                            }

                        }
                    }
                }
                
            }
            
        }else{
            println!("Erreur");
        }
    }
}

pub fn explicationmenacequestion(plan: &Vec<Op>,menace: &DMatrix<i32>,support : &DMatrix<i32> , step1: i32, step2:i32)->bool{
	//dijkstra( plan, ground);
	let length=plan.len();
    let l2=length as u32;
    let s1=step1 as usize;
    let s2=step2 as usize;
    let m=menace.get((s1,s2));
    let mut b=false;
    if !m.is_none(){
        if *m.unwrap() == 0 {
            b = false;
        }else if *m.unwrap() == 1{
            b = true;
        }else if *m.unwrap() == -1{
           b=true;
        }else if *m.unwrap() == -2{
            b=true;            
        }else{
            println!("Erreur");
        }
    }
    b
}

pub fn explicationmenacequestiondetail(plan: &Vec<Op>,menace: &DMatrix<i32>,support : &DMatrix<i32> , /*ground : &GroundProblem,*/ step1: i32, step2:i32)->Option<(usize,usize,Option<usize>)>{
	//dijkstra( plan, ground);
	let length=plan.len();
    let l2=length as u32;
    let s1=step1 as usize;
    let s2=step2 as usize;
    let m=menace.get((s1,s2));
    if !m.is_none(){
        if *m.unwrap() == 0 {
            return None
        }else if *m.unwrap() == 1{
            return Some((s1, s2, None))
        }else if *m.unwrap() == -1{
            return Some((s1,s2,None))
        }else if *m.unwrap() == -2{
            //on regarde les support de s2
            let row=support.column(s2);
            for index in 0..l2+1{
                let i = index as usize;
                let support=row.get(i);
                if !support.is_none(){
                    if *support.unwrap()==1 {
                        //on regarde quel support est lié à la menace
                        let ms=menace.get((s1,i));
                        if !ms.is_none() {
                            if *ms.unwrap() == -1{
                               return Some((s1,s2, Some(i)))
                            }

                        }
                    }
                }
                
            }
            
        }else{
            println!("Erreur valeur matrice");
            return None
        }
    }else{
        println!("Erreur init matrice");
    }
    return None
}

pub fn explication2etape(plan: &Vec<Op>,menace: &DMatrix<i32>,support : &DMatrix<i32> , /*ground : &GroundProblem,*/ step1: i32, step2:i32){
    println!("lien entre les étape {} et {}",step2,step1);
    if step1 > step2 {
        let nec=explicationsupport(plan, support, /*ground,*/ step1, step2);
        nec.affiche();
    }else{
        let nec=explicationsupport(plan, support, /*ground,*/ step2, step1);
        nec.affiche();
    }
    explicationmenace(plan, menace, support, /*ground,*/ step1, step2);
    explicationmenace(plan, menace, support, /*ground,*/ step2, step1);
}


pub fn choixpredicat(i : usize,initial_state: &State)-> SVId {
    let mut l = initial_state.literals();
    let n=l.nth(i);
    match n {
        None => choixpredicat(i-1,initial_state),
        Some(n)=>n.var(),
    }
}

pub fn choixpredaction(i:usize,plan: &Vec<Op>,ground: &GroundProblem)->Vec<SVId>{
    //pas bon il faut rechercher l'id d'un symbol par ex: move  car operztor = 1 move instancié genre move rooma roomb
    //let init=&ground.initial_state;
    let ops=&ground.operators;
    //let goals=&ground.goals;
    let a =*plan.get(i).unwrap();
    let ap=ops.preconditions(a);
    let ae=ops.effects(a);
    let mut out = Vec::new();
    for i in ap{
        let n =*i;
        let v =n.var();
        out.push(v);
    }
    for i in ae{
        let n =*i;
        let v =n.var();
        out.push(v);
    }
    out
}

pub fn choixpredaction2(i:usize,plan: &Vec<Op>,ground: &GroundProblem)->Vec<SVId>
{
    //pas bon il faut rechercher l'id d'un symbol par ex: move  car operztor = 1 move instancié genre move rooma roomb
    //let init=&ground.initial_state;
    let ops=&ground.operators;
    //let goals=&ground.goals;
    let mut out = Vec::new();

    let a =*plan.get(i).unwrap();
    let action = ops.name(a).get(0).unwrap();
    /*let ap=ops.preconditions(a);
    let ae=ops.effects(a);
    for i in ap{
        let n =*i;
        let v =n.var();
        out.push(v);
    }
    for i in ae{
        let n =*i;
        let v =n.var();
        out.push(v);
    }*/
    for i in ops.iter(){
        let test= ops.name(i).get(0);
        if !test.is_none(){
            if action==test.unwrap(){
                let ae=ops.effects(i);
                for i in ae{
                    let n =*i;
                    let v =n.var();
                    out.push(v);
                }
            }
        }
    }
    println!("nb d'action sélectionné predaction2 {}",out.len());
    out
}

/*
********************************
faire question avec move :
Besoin de world pour extraire id de move puis utiliser cette id dans ops
****************************
*/


pub fn choixpredaction3/*<T,I>*/(action:String,plan: &Vec<Op>,ground: &GroundProblem,wo: &SymbolTable<String,String>)->Vec<SVId>
/*where
    T: Clone + Eq + Hash + Display,
    I: Clone + Eq + Hash + Display,*/
{
    //let init=&ground.initial_state;
    let ops=&ground.operators;
    //let goals=&ground.goals;
    let mut out = Vec::new();
    //let table=&wo.table;
    let var = /*table*/wo.id(&action);
    println!("{:?}",var);
    if var.is_none() {
        println!("Erreur, action selectionné non trouvé");
    }
    else{
        for i in ops.iter(){
            let test= ops.name(i).get(0);
            if !test.is_none(){
                if var.unwrap() == *test.unwrap(){
                    //println!(" essai {:?}",*test.unwrap());
                    let ae=ops.effects(i);
                    for i in ae{
                        let n =*i;
                        let v =n.var();
                        out.push(v);
                    }
                }
            }
        }
    }
    println!("nb de SVId sélectionné predaction3 {}",out.len());
    out
}

pub fn dijkstrapoids(plan : &Vec<Op>,ground: &GroundProblem,mat : &DMatrix<i32>,predicat: &Vec<SVId>,infini:i32)->Vec<Necessaire>{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    //let infini=(l2*l2) as i32;

    //let dd=l2+1;
    //let plan2=plan.clone();
    let plan3=plan.clone();
    let mut matrice=mat.clone();
    let mut atraite=Vec::new();
    let mut traite=Vec::new();

    //
    // mettre les poids ici avec prédicats
    //
    for a in 0..l2+1{
        for b in 0..l2+1{
            let i=a as usize;
            let j=b as usize;
            let m=matrice.get((i,j));
            if !m.is_none(){
                if *m.unwrap() == 1{
                    let support=plan.get(i);
                    let action=plan.get(j);
                    if !support.is_none() && !action.is_none(){
                        let s = *support.unwrap();
                        let a= *action.unwrap();
                        let precon = ops.preconditions(a);
                        let effet = ops.effects(s);
                        for pre in precon{
                            for eff in effet{
                                for p in predicat{
                                    if pre.var() == *p && eff.var()== *p{
                                            matrice[(i,j)]=infini;
                                    }
                                }
                                
                            }
                        }

                    }
                    
                }
            }
        }

    }
    //affichagematrice(&matrice);
    //dijkstra

//pas touche
    let cause = causalitegoals(plan3,init,ops,goals);
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2*l2+1);
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
                            let n=*matrice.get((i,ind)).unwrap();
                            let n = n as u32;
                            let nec=newnec(res.opnec(),somme.nec(),newchemin,somme.long()+n );
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


pub fn dijkstrapoidsavantage(plan : &Vec<Op>,ground: &GroundProblem,mat : &DMatrix<i32>,predicat: &Vec<SVId>,infini:i32)->Vec<Necessaire>{
    let init=&ground.initial_state;
    let ops=&ground.operators;
    let goals=&ground.goals;
    let length=plan.len();
    let l2=length as u32;
    //let infini=(l2*l2) as i32;

    //let dd=l2+1;
    let plan3=plan.clone();
    let mut matrice=mat.clone();
    let mut atraite=Vec::new();
    let mut traite=Vec::new();

    //
    // mettre les poids ici avec prédicats
    //
    for a in 0..l2+1{
        for b in 0..l2+1{
            let i=a as usize;
            let j=b as usize;
            let m=matrice.get((i,j));
            if !m.is_none(){
                if *m.unwrap() == 1 {
                    matrice[(i,j)]=infini;
                    let support=plan.get(i);
                    let action=plan.get(j);
                    if !support.is_none() && !action.is_none(){
                        let s = *support.unwrap();
                        let a= *action.unwrap();
                        let precon = ops.preconditions(a);
                        let effet = ops.effects(s);
                        for pre in precon{
                            for eff in effet{
                                for p in predicat{
                                    if pre.var() == *p && eff.var()== *p{
                                        matrice[(i,j)]=1;
                                    }
                                }
                                
                            }
                        }

                    }
                    
                }
            }
        }

    }
    //affichagematrice(&matrice);
    //dijkstra

//pas touche
    let cause = causalitegoals(plan3,init,ops,goals);
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2*l2+1);
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
                            let n=*matrice.get((i,ind)).unwrap();
                            let n = n as u32;
                            let nec=newnec(res.opnec(),somme.nec(),newchemin,somme.long()+n );
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



/***
ATTENTION Step 1 est l'étape supporté et step 2 la supportante
*******/
pub fn supportindirectavantagepoid(step1: i32, step2:i32, plan: &Vec<Op>, ground: &GroundProblem,support : &DMatrix<i32> , predicat: &Vec<SVId>,infini:i32 )->Necessaire{
    //dijkstra( plan, ground);
    let init=&ground.initial_state;
    let ops=&ground.operators;
	let length=plan.len();
	let mut atraite=Vec::new();
	let mut traite=Vec::new();
    let l2=length as u32;
    
    //let plan3=plan.clone();
    let mut matrice=support.clone();

    //
    // mettre les poids ici avec prédicats
    //
    for a in 0..l2+1{
        for b in 0..l2+1{
            let i=a as usize;
            let j=b as usize;
            let m=matrice.get((i,j));
            if !m.is_none(){
                if *m.unwrap() == 1 {
                    matrice[(i,j)]=infini;
                    let support=plan.get(i);
                    let action=plan.get(j);
                    if !support.is_none() && !action.is_none(){
                        let s = *support.unwrap();
                        let a= *action.unwrap();
                        let precon = ops.preconditions(a);
                        let effet = ops.effects(s);
                        for pre in precon{
                            for eff in effet{
                                for p in predicat{
                                    if pre.var() == *p && eff.var()== *p{
                                        matrice[(i,j)]=1;
                                    }
                                }
                                
                            }
                        }

                    }
                    
                }
            }
        }

    }

//init
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2*l2+1);
        //if mene à step1
		if count == step1 {
			nec = newnecgoal(step);
		}      
        atraite.push(nec);
        count=count+1;
    }

 //Dijk 
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
        atraite.remove(index);
        let sommec=somme.clone();
        traite.push(sommec);
        //examine tous les successeurs y du sommet x qui ne sont pas traités
        for i in 0..length+1{
            let b=i as i32;
            let ind=somme.opnec().numero() as usize;
            if matrice[(i,ind)]!=0{
                //println!("essai entrée dijk 0");
                let mut newatraite = Vec::new();
                for res in atraite{
					//println!("essai entrée dijk {}, {}",res.opnec().numero(),b);
                    if res.opnec().numero()==b{//ici pb
                        //println!("essai entrée dijk 1");
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
    let s2=step2 as usize;
    let mut step;
    if !(plan.get(s2).is_none()){
        step =newresume(*plan.get(s2).unwrap(),step2);
    }else{
        step=goalresume(step2);
    }
	let mut nec =initnec(step,l2+1);
	for i in traite{
		if i.opnec().numero()==step2{
			nec= i;
		}
	}
	nec
}

pub fn supportindirectpoid(step1: i32, step2:i32, plan: &Vec<Op>, ground: &GroundProblem,support : &DMatrix<i32> , predicat: &Vec<SVId>,infini:i32 )->Necessaire{
    //dijkstra( plan, ground);
    let init=&ground.initial_state;
    let ops=&ground.operators;
	let length=plan.len();
	let mut atraite=Vec::new();
	let mut traite=Vec::new();
    let l2=length as u32;
    
    //let plan3=plan.clone();
    let mut matrice=support.clone();

    //
    // mettre les poids ici avec prédicats
    //
    for a in 0..l2+1{
        for b in 0..l2+1{
            let i=a as usize;
            let j=b as usize;
            let m=matrice.get((i,j));
            if !m.is_none(){
                if *m.unwrap() == 1{
                    let support=plan.get(i);
                    let action=plan.get(j);
                    if !support.is_none() && !action.is_none(){
                        let s = *support.unwrap();
                        let a= *action.unwrap();
                        let precon = ops.preconditions(a);
                        let effet = ops.effects(s);
                        for pre in precon{
                            for eff in effet{
                                for p in predicat{
                                    if pre.var() == *p && eff.var()== *p{
                                            matrice[(i,j)]=infini;
                                    }
                                }
                                
                            }
                        }

                    }
                    
                }
            }
        }

    }
    //affichagematrice(&matrice);

//init
    let mut count=0;
    let plan2=plan.clone();
    for i in plan2{
        let step =newresume(i,count);
        let mut nec =initnec(step,l2*l2+1);
        //if mene à step1
		if count == step1 {
			nec = newnecgoal(step);
		}      
        atraite.push(nec);
        count=count+1;
    }

 //Dijk 
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
        atraite.remove(index);
        let sommec=somme.clone();
        traite.push(sommec);
        //examine tous les successeurs y du sommet x qui ne sont pas traités
        for i in 0..length+1{
            let b=i as i32;
            let ind=somme.opnec().numero() as usize;
            if matrice[(i,ind)]!=0{
                //println!("essai entrée dijk 0");
                let mut newatraite = Vec::new();
                for res in atraite{
					//println!("essai entrée dijk {}, {}",res.opnec().numero(),b);
                    if res.opnec().numero()==b{//ici pb
                        //println!("essai entrée dijk 1");
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
    let s2=step2 as usize;
    let mut step;
    if !(plan.get(s2).is_none()){
        step =newresume(*plan.get(s2).unwrap(),step2);
    }else{
        step=goalresume(step2);
    }
	let mut nec =initnec(step,l2+1);
	for i in traite{
		if i.opnec().numero()==step2{
			nec= i;
		}
	}
	nec
}


//Abstraction

//regrouper action support l'une de l'autre, questions on prend op (ex gripper les move grouperai mais pas pick drop) ou action complète (les move grouperai ensemble et les pick drop entre eux aussi)
pub fn abstractionop(support: &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem )->Vec<Vec<Op>>{
    let row=support.nrows();
    let col=support.ncols();


    //Compter nombre Op
    let mut nbop=0;
    let mut v=Vec::new();
    let mut out=Vec::new();
    for i in plan{
        if v.is_empty(){
            v.push(*i);
            nbop=nbop+1;
            //println!("comptage");
        }else{
            let mut notin=true;
            for ope in &v{
                if *ope == *i {
                    
                    notin=false;
                }
            }
            if notin {
                v.push(*i);
                nbop=nbop+1;   
            }
        }
    }
    //regroupement etapes selon l'Op
    let mut matrice = DMatrix::from_diagonal_element(nbop,nbop,0);
    for ligne in 0..row-2{
        for colonnes in 0..col-2{
            if support[(ligne,colonnes)] == 1{
                let op1 = *plan.get(ligne).unwrap();
                let op2 = *plan.get(colonnes).unwrap();
                let mut placeop1=0;
                let mut placeop2=0;
                let mut count=0;
                for op in &v{
                    if *op == op1{
                        placeop1=count;
                    }else if *op == op2{
                        placeop2=count;
                    }
                    count=count+1;
                }
                matrice[(placeop1,placeop2)]=1;  
            }
        }
    }
    affichagematrice(&matrice);
    //regarder les liens entre les Op si mat(i,j)=1=mat(j,i)=>un groupe
    for l in 0..nbop-1{
        for c in l..nbop-1{
            if matrice[(l,c)]==1{
                //println!("test");
                if matrice[(c,l)]==1{
                    //println!("groupage");
                    let mut groupe = Vec::new();
                    groupe.push(*v.get(c).unwrap());
                    groupe.push(*v.get(l).unwrap());
                    out.push(groupe);
                }
            }
        }
    }
    out
}

pub fn abstractionaction(support: &DMatrix<i32>, plan: &Vec<Op>, ground: &GroundProblem, symbol : &SymbolTable<String,String> )->Vec<Vec<SymId>>{
    /*let var = symbol.id(&action);
    if var.is_none() {
        println!("Erreur, action selectionné non trouvé");
    }
    else{

    }*/

    let row=support.nrows();
    let col=support.ncols();


    //Compter nombre D'action (SymId même principe que Op)
    let mut nbop=0;
    let mut v=Vec::new();
    let mut out=Vec::new();
    for i in plan{
        if v.is_empty(){
            let action = &ground.operators.name(*i);
            v.push(action[0]);
            nbop=nbop+1;
        }else{
            let mut notin=true;
            for ope in &v{
                let action=&ground.operators.name(*i);
                if *ope == action[0] {
                    
                    notin=false;
                }
            }
            if notin {
                let action = &ground.operators.name(*i);
                v.push(action[0]);
                nbop=nbop+1;   
            }
        }
    }
    //regroupement etapes selon l'action
    let mut matrice = DMatrix::from_diagonal_element(nbop,nbop,0);
    for ligne in 0..row-2{
        for colonnes in 0..col-2{
            if support[(ligne,colonnes)] == 1{
                let op1 = *plan.get(ligne).unwrap();
                let action1=&ground.operators.name(op1);
                let op2 = *plan.get(colonnes).unwrap();
                let action2=&ground.operators.name(op2);
                let mut placeop1=0;
                let mut placeop2=0;
                let mut count=0;
                for op in &v{
                    if *op == action1[0]{
                        placeop1=count;
                    }
                    if *op == action2[0]{
                        placeop2=count;
                    }
                    count=count+1;
                }
                matrice[(placeop1,placeop2)]=1;  
            }
        }
    }
    affichagematrice(&matrice);
    //regarder les liens entre les Op si mat(i,j)=1=mat(j,i)=>un groupe
    for l in 0..nbop{
        for c in l..nbop{
            if matrice[(l,c)]==1{
                //println!("test");
                if matrice[(c,l)]==1{
                    //println!("groupage");
                    let mut groupe = Vec::new();
                    groupe.push(*v.get(c).unwrap());
                    groupe.push(*v.get(l).unwrap());
                    out.push(groupe);
                }
            }
        }
    }
    out
}

//Synchronisation

pub fn coordination(parametre : &Vec<String>,plan : &Vec<Op>,ground: &GroundProblem,symbol: &SymbolTable<String,String>)->HashMap<SymId,Vec<Op>>{
    let mut h = HashMap::new();
    for param in parametre{
        let id= symbol.id(param);
        if id.is_none(){
            println!("erreur entrée paramètre");
        }
        else{
            if h.get_mut(&id.unwrap()).is_none(){
                let mut  v=Vec::new();
                h.insert(id.unwrap(),v);
            }
        }
    }
    for op in plan{
        let opid = ground.operators.name(*op);
        for id in opid{
            if !h.get_mut(id).is_none(){
                let ajout = h.get_mut(id).unwrap();
                ajout.push(*op);
            }
        }
    }
    h
}

pub fn affichagecoordination<T,I : Display>(h: &HashMap<SymId,Vec<Op>>, ground: &GroundProblem, wo: &World<T,I> ){
    for (i,vec) in h.iter(){
        let vecinter = vec![*i];
        let slice = &vecinter[..];
        println!("Le paramètre {} est utilisé dans :",wo.table.format(slice));
        for op in vec{
            println!("  l'opérateur {}",wo.table.format(&ground.operators.name(*op)));
        }
    }
}

pub fn synchronisation/*<T,I : Display>*/(h: &HashMap<SymId,Vec<Op>>,support: &DMatrix <i32>,plan : &Vec<Op>, /*ground: &GroundProblem, wo: &World<T,I>*/)->Vec<Resume>{
    let mut out =Vec::new();
    let mut count = 0;
    let t = plan.len();
    for i in plan{
        let mut groupe :Option<SymId>=None;
        for (key,vec) in h.iter(){
            for op in vec{
                if *op == *i {
                    groupe = Some(*key);
                }
            }
        }
        for sup in 0..t {
            if support[(count,sup)]==1{
                for (key,vec) in h.iter(){
                    for op in vec{
                        if *op == *plan.get(sup).unwrap() {
                            if groupe.is_some(){
                               if *key != groupe.unwrap(){
                                    let num = count as i32;
                                    let step=newresume(*i,num);
                                    out.push(step);
                                } 
                            }
                            
                        }
                    }
                } 
            }

        }
        count=count+1;
    }
    out
}

//Mise au poids paramètrique

//en réutilisant coordination
pub fn poidsparametredesavantage(poids : i32, support: &DMatrix <i32>,h: &HashMap<SymId,Vec<Op>>, plan: &Vec<Op>, ground: &GroundProblem )->DMatrix <i32>{
    let mut count=0;
    let mut supportpoids=support.clone();
    let t = plan.len();
    for i in plan{
        let mut paramutile = false;
        for (key,vec) in h.iter(){
            for op in vec{
                if *op == *i {
                    paramutile = true;
                }
            }
        }
        if paramutile {
            for sup in 0..t+1 {
                if support[(count,sup)]==1{
                    supportpoids[(count,sup)]=poids;
                }

            }
        }
        count= count+1;
    }
    supportpoids
}

pub fn poidsparametreavantage(poids : i32, support: &DMatrix <i32>,h: &HashMap<SymId,Vec<Op>>, plan: &Vec<Op>, ground: &GroundProblem)->DMatrix <i32>{
    let mut count=0;
    let mut supportpoids=support.clone();
    let t = plan.len();
    for i in plan{
        let mut paramutile = false;
        for (key,vec) in h.iter(){
            for op in vec{
                if *op == *i {
                    paramutile = true;
                }
            }
        }
        if !paramutile {
            for sup in 0..t+1 {
                if support[(count,sup)]==1{
                    supportpoids[(count,sup)]=poids;
                }

            }
        }
        count= count+1;
    }
    supportpoids
}


pub fn coordinationmultiple(parametre : &Vec<String>,plan : &Vec<Op>,ground: &GroundProblem,symbol: &SymbolTable<String,String>)->Vec<Op>{
    let t = parametre.len();
    let mut paramid= Vec::new();
    let mut out = Vec::new();
    for param in parametre{
        let id= symbol.id(param);
        if id.is_none(){
            println!("erreur entrée paramètre");
        }
        else{
            if !paramid.contains(&id.unwrap()){
                paramid.push(id.unwrap());
            }
        }
    }
    for op in plan{
        let opid = ground.operators.name(*op);
        let mut count = 0;
        for id in opid{
            if paramid.contains(id){
                count=count+1;
            }
        }
        if count == t {
            out.push(*op);
        }
    }
    out
}

pub fn liencoormultisynchro(liste : &Vec<Op>,parametre : &Vec<String>,symbol: &SymbolTable<String,String>)->HashMap<SymId,Vec<Op>>{
    let mut h = HashMap::new();
    let p=parametre.get(0).unwrap();
    let s=symbol.id(p);
    if s.is_some(){
       h.insert(s.unwrap(),liste.clone()); 
    }
    h
}
//Tentative goulot avec flot max / coupe min
/*
pub fn chaineameliorante(support : DMatrix<i32>,flotprec :DMatrix<i32>)->bool{
    let mut file=Vec::new();
    let mut marquer=Vec::new();
    let taille=support.nrows();
    /*flot=Vec::with_capacity(taille-2);
    for i in 0..taille-2{
        flot.push(0);
    }*/
    let mut flot = flotprec.clone();
    //let mut flot = DMatrix::from_diagonal_element(taille-1,taille-1,0);
    /*for l in 0..taille-1{
        for c in 0..taille-1{
            if support[]
        }
    }*/

    file.push(0);
    while !file.is_empty(){
        let n=file.remove(0);
        for y in 0..taille-1{
            if support[(n,y)]==1{
                //regarder si y est marqué et si on peut améliorer son flot
                let mut m = false;
                for mark in &marquer{
                    if support[(n,y)]==*mark{
                        m=true;
                    }
                } 
                if m && ( flot[(n,y)]<support[(n,y)] ){
                    let y32= y as i32;
                    marquer.push(y32);
                    file.push(y);  
                }
            }//reste à faire premier rouge
            if support[(y,n)]==1{
                let mut m = false;
                for mark in &marquer{
                    if support[(y,n)]==*mark{
                        m=true;
                    }
                }
                if m && ( 0<flot[(y,n)] ){
                    let y32= y as i32;
                    marquer.push(y32);
                    file.push(y); 
                }
            }
        }
    }
    let t = taille as i32;
    let u=t-1;
    if marquer.contains(&u){
        true
    }else{
        false
    }

}

pub fn fordfulkerson(support : DMatrix<i32>)->DMatrix<i32>{
    let mut flot = DMatrix::from_diagonal_element(taille-1,taille-1,0);
    let mut ecart = DMatrix::from_diagonal_element(taille-1,taille-1,0);
    while chaineameliorante(support,flot) {
        for l in 0..taille-1{
            for c in 0..taille-1{
                ecart[(l,c)]=support[(l,c)]-flot[(l,c)];
            }
        }
        if  chaineameliorante(support,flot){

        }
    }
}*/