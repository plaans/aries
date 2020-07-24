use crate::classical::state::*;


//Pour lier Op et une etape 
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Resume{
    opkey: Option<Op>,
    etape: i32,
}

impl Resume{
    pub fn op(&self)-> Option<Op> {
        self.opkey
    }
    
    pub fn numero(&self)-> i32{
        self.etape
    }

    

}
pub fn newresume(ope: Op,num: i32)->Resume{
        Resume {
            opkey : Some(ope),
            etape : num,
        }
    }

pub fn defaultresume()->Resume{
        Resume{
            opkey : None,
            etape : -1,
        }
    }

pub fn goalresume(num: i32)->Resume{
        Resume{
            opkey : None,
            etape : num,
        }
    }

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Necessaire{
    operateur: Resume,
    nec: bool,
    chemin: Option<Vec<Resume>>,
    longueur: u32,
}

impl Necessaire{
    pub fn opnec(&self)->Resume{self.operateur}
    pub fn nec(&self)->bool{self.nec}
    pub fn chemin(&self)->Option<Vec<Resume>>{self.chemin.clone()}
    pub fn long(&self)->u32{self.longueur}
    pub fn presence(&self,res:Resume)->bool{self.operateur==res}

    pub fn affiche (&self){
        println!(" l'étape {} est nécessaire {} dans le chemin de longueur {} composé par :"/*,self.opnec().op()*/,self.opnec().numero(),self.nec,self.long());
        if self.chemin().is_none(){println!("pas de chemin");}
        else{
            for res in self.chemin().unwrap(){
                println!(" l'étape {}", res.numero());
            }
        }
    }
}
pub fn newnec(op:Resume, b:bool, way:Vec<Resume>, l:u32)->Necessaire{
    Necessaire{
        operateur:op,
        chemin:Some(way),
        nec:b,
        longueur:l,
    }
}

pub fn newnecgoal(op:Resume)->Necessaire{
    Necessaire{
        operateur:op,
        nec:true,
        chemin:None,
        longueur: 0,
    }
}

pub fn newnecess(op:Resume)->Necessaire{
    Necessaire{
        operateur:op,
        nec:false,
        chemin:None,
        longueur: 0,
    }
}

pub fn initnec(op:Resume, inf:u32)->Necessaire{
    Necessaire{
        operateur:op,
        nec:false,
        chemin:None,
        longueur: inf,
    }
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub struct Unique{
    operateur : Op,
    unicite : bool,
}

impl Unique{
    pub fn operateur(&self)->Op{self.operateur}
    pub fn unicite(&self)->bool{self.unicite}
    pub fn duplicite(&mut self){
        self.unicite=false;
    }
}

pub fn newunique(ope:Op)->Unique{
    Unique{
        operateur:ope,
        unicite : true,
    }
}

pub struct Obligationtemp{
    ope1 : Op,
    etape1: i32,
    ope2: Op,
    etape2: i32
}

impl Obligationtemp{
    pub fn operateur(&self)->(Op,Op){(self.ope1,self.ope2)}
    pub fn etape(&self)->(i32,i32){(self.etape1,self.etape2)}
    pub fn premiereetape(&self)->(Op,i32){
        if self.etape2>self.etape1{
            (self.ope1,self.etape1)
        }else{
            (self.ope2,self.etape2)
        }
    }
    pub fn deuxiemeetape(&self)->(Op,i32){
        if self.etape1>self.etape2{
            (self.ope1,self.etape1)
        }else{
            (self.ope2,self.etape2)
        }
    }
    pub fn affichage(&self){
         println!(" l'étape {} et l'étape {} ne sont pas inversible",self.etape1,self.etape2);
    }
}

pub fn newot(ope:Op,step:i32,oper:Op,next:i32)->Obligationtemp{
    Obligationtemp{
        ope1 : ope,
        etape1 : step,
        ope2 : oper,
        etape2 : next,
    }
}

#[derive(PartialEq)]
pub enum Parallelisable {
    Oui,
    Non_menace {origine:usize, vers:usize},
    Non_support {origine:usize,vers:usize},
}


pub fn originenonp(p:Parallelisable)->usize{
    match p{
        Parallelisable::Non_menace {origine,vers}=> origine,
        Parallelisable::Non_support {origine,vers}=> origine,
        _=>{
            println!("Les 2 étapes sont parallelisable");
            0
        }
    }
}

pub fn ciblenonp(p:Parallelisable)->usize{
    match p{
        Parallelisable::Non_menace {origine,vers}=> vers,
        Parallelisable::Non_support {origine,vers}=> vers,
        _=>{
            println!("Les 2 étapes sont parallelisable");
            0
        }
    }
}

pub enum Parallelisabledetail {
    Oui,
    Menace_Apres {origine:usize, vers:usize},
    Menace_Avant {origine:usize, vers:usize,supportconcern:Option<usize>},
    Support_Direct {origine:usize,vers:usize},
    Support_Indirect {origine:usize,vers:usize,chemin:Vec<Resume>},
}

pub fn originenonpad(p:Parallelisabledetail)->usize{
    match p{
        Parallelisabledetail::Menace_Apres {origine,vers}=> origine,
        Parallelisabledetail::Menace_Avant {origine,vers,supportconcern}=> origine,
        Parallelisabledetail::Support_Direct {origine,vers}=> origine,
        Parallelisabledetail::Support_Indirect {origine,vers,chemin}=> origine,
        _=>{
            println!("Les 2 étapes sont parallelisable");
            0
        }
    }
}

pub fn ciblenonpad(p:Parallelisabledetail)->usize{
    match p{
        Parallelisabledetail::Menace_Apres {origine,vers}=> vers,
        Parallelisabledetail::Menace_Avant {origine,vers,supportconcern}=> vers,
        Parallelisabledetail::Support_Direct {origine,vers}=> vers,
        Parallelisabledetail::Support_Indirect {origine,vers,chemin}=> vers,
        _=>{
            println!("Les 2 étapes sont parallelisable");
            0
        }
    }
}
//match à refaire pour avoir sortie cohérente refaire menace avant en vec.
pub fn pad_detail(p:Parallelisabledetail)->Vec<Option<usize>>{
    match p{
        Parallelisabledetail::Menace_Avant {origine,vers,supportconcern}=> {let mut n =Vec::new();
                                                                                                        n.push(supportconcern);
                                                                                                        n
                                                                                                    },
        Parallelisabledetail::Support_Indirect {origine,vers,chemin}=> { let mut v=Vec::new();
                                                                                                    for n in chemin{
                                                                                                        let i=n.numero() as usize;
                                                                                                        v.push(Some(i));
                                                                                                    }
                                                                                                    v
                                                                                                },
        _ => {
            println!(" Pas de détails supplémentaire");
            let mut v=Vec::new();
            v.push(None);
            v
        }
    }
}
