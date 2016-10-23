/* Main library file
 *
 * Copyright (C) 2016 TyanNN <TyanNN@cocaine.ninja>
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program; if not, write to the Free Software Foundation, Inc.,
 * 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.
*/

//! Эта библиотека предназначена для
//! взаимодействия с [табуном](https://tabun.everypony.ru)
//! (и потенциально прочими сайтами на лайвстрите), так как
//! API у них нет.
//!
//! Весь интерфейс находится в [`TClient`](struct.TClient.html), хотя на самом деле
//! разнесён по нескольким файлам.
//!
//! Большинство функций ~~нагло украдены~~ портированы с [`tabun_api`](https://github.com/andreymal/tabun_api)

extern crate hyper;
extern crate select;
extern crate regex;
extern crate url;
extern crate multipart;
extern crate unescape;
#[macro_use] extern crate hado;

use std::fmt::Display;
use std::str::FromStr;

use regex::Regex;

use std::collections::HashMap;

use hyper::client::Client;
use hyper::client::request::Request;
use hyper::header::{CookieJar,SetCookie,Cookie,ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper::status::StatusCode;

use url::form_urlencoded;

use multipart::client::Multipart;

use std::io::Read;

use select::document::Document;
use select::predicate::{Class, Name, And, Attr};

macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

///Макро для парса строк и возврата Result,
///парсит st указанным regex, затем вынимает группу номер num
///и парсит в typ
macro_rules! parse_text_to_res(
    { $(regex => $regex:expr, st => $st:expr, num => $n:expr, typ => $typ:ty)+ } => {
        {
            $(
                match hado! {
                    reg <- Regex::new($regex).ok();
                    captures <- reg.captures($st);
                    at <- captures.at($n);
                    at.parse::<$typ>().ok() } {
                        Some(x) => Ok(x),
                        None    => unreachable!()
                    }
            )+
        }
    };
);

///Макро для удобного unescape
macro_rules! unescape(
    { $($x:expr)+ } => {
        {
            $(
                match unescape::unescape($x) {
                    Some(x) => x,
                    None    => unreachable!()
                }
             )+
        }
    };
);

///Макрос для возвращения ошибок парсинга
macro_rules! try_to_parse {
    ( $expr: expr ) => {
        match $expr {
            Some(x) => x,
            None => return Err(TabunError::ParseError(
                String::from(file!()), line!(), String::from("Cannot parse page")
            )),
        }
    };
    ( $expr: expr, $msg: expr ) => {
        match $expr {
            Some(x) => x,
            None => return Err(TabunError::ParseError(
                String::from(file!()), line!(), String::from($msg)
            )),
        }
    };
}

mod comments;
mod posts;
mod talks;

//Перечисления

#[derive(Debug)]
pub enum TabunError {
    ///На случай `Hacking attempt!`
    HackingAttempt,

    ///Ошибка с названием и описанием,
    ///обычно соответствует табуновским
    ///всплывающим сообщениям
    Error(String,String),

    ///Ошибка с номером, вроде 404 и 403
    NumError(StatusCode),

    ///Ошибка HTTP или ошибка сети, которая может быть при плохом интернете
    ///или лежачем Табуне
    IoError(hyper::error::Error),

    ///Ошибка парсинга страницы. Скорее всего будет возникать после изменения
    ///вёрстки Табуна, поэтому имеет смысл сообщать об этой ошибке
    ///разработчикам
    ParseError(String, u32, String)
}

///Тип комментария для ответа
pub enum CommentType {
    ///Комментарий к посту
    Post,

    ///Ответ на личное сообщение
    Talk
}

//Структуры

///Клиент табуна
pub struct TClient<'a> {
    pub name:               String,
    pub security_ls_key:    String,
    client:                 Client,
    cookies:                CookieJar<'a>,
}

#[derive(Debug,Clone)]
pub struct Comment {
    pub body:       String,
    pub id:         u32,
    pub author:     String,
    pub date:       String,
    pub votes:      i32,
    pub parent:     u32,
    pub post_id:    u32,
    pub deleted:    bool
}

#[derive(Debug,Clone)]
pub struct Post {
    pub title:          String,
    pub body:           String,
    pub date:           String,
    pub tags:           Vec<String>,
    pub comments_count: u32,
    pub author:         String,
    pub id:             u32,
}

#[derive(Debug,Clone)]
pub struct EditablePost {
    pub title:          String,
    pub body:           String,
    pub tags:           Vec<String>,
}

///Блоги из списка блогов в [профиле](struct.UserInfo.html)
#[derive(Debug,Clone)]
pub struct InBlogs {
    ///Созданные пользователем блоги
    pub created: Vec<String>,

    ///Блоги, в которых пользователь является администратором
    pub admin: Vec<String>,

    ///Блоги, в которых пользователь является модератором
    pub moderator: Vec<String>,

    ///Блоги, в которых пользователь состоит
    pub member: Vec<String>
}


///Профиль некоторого пользователя
#[derive(Debug,Clone)]
pub struct UserInfo {
    pub username:       String,
    pub realname:       String,

    ///Силушка
    pub skill:          f32,
    pub id:             u32,

    ///Кармочка
    pub rating:         f32,

    ///URL картинки, иногда с `//`, иногда с `https://`
    pub userpic:        String,
    pub description:    String,

    ///Информация вроде даты рождения и последнего визита,
    ///поля называются как на сайте
    pub other_info:     HashMap<String,String>,

    ///Блоги, которые юзер создал/состоит в них/модерирует
    pub blogs:          InBlogs,

    ///Кол-во публикаций
    pub publications:   u32,

    ///Кол-во избранного
    pub favourites:     u32,

    ///Кол-во друзей
    pub friends:        u32
}

///Диалог в личных сообщениях
#[derive(Debug,Clone)]
pub struct Talk {
    pub title:  String,
    pub body:   String,

    ///Участники
    pub users:  Vec<String>,
    pub comments: HashMap<u32, Comment>,
    pub date:   String
}

///Список личных сообщений
#[derive(Debug,Clone)]
pub struct TalkItem {
    pub id: u32,
    pub title:  String,
    pub users:  Vec<String>,
}

//Реализации

impl From<StatusCode> for TabunError {
    fn from(x: StatusCode) -> Self {
        TabunError::NumError(x)
    }
}

impl From<hyper::error::Error> for TabunError {
    fn from(x: hyper::error::Error) -> Self {
        TabunError::IoError(x)
    }
}

impl From<std::io::Error> for TabunError {
    fn from(x: std::io::Error) -> Self {
        TabunError::IoError(hyper::Error::Io(x))
    }
}

impl Display for Comment {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Comment({},\"{}\",\"{}\")", self.id, self.author, self.body)
    }
}

impl Display for Post {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Post({},\"{}\",\"{}\")", self.id, self.author, self.body)
    }
}

impl Display for UserInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "UserInfo({},\"{}\",\"{}\")", self.username, self.skill, self.rating)
    }
}

///URL сайта. Ибо по идее может работать и с другими штуками на лайвстрите
pub const HOST_URL: &'static str = "https://tabun.everypony.ru";

pub type TabunResult<T> = Result<T,TabunError>;

impl<'a> TClient<'a> {

    ///Входит на табунчик и сохраняет LIVESTREET_SECURITY_KEY,
    ///если логин или пароль == None - анонимус.
    ///
    ///# Examples
    ///```no_run
    ///let mut user = libtabun::TClient::new("логин","пароль");
    ///```
    pub fn new<T: Into<Option<&'a str>>>(login: T, pass: T) -> TabunResult<TClient<'a>> {
        let mut user = TClient{
            name:               String::new(),
            security_ls_key:    String::new(),
            client:             Client::new(),
            cookies:            CookieJar::new(format!("{:?}",std::time::SystemTime::now()).as_bytes()),
        };

        let ls_key_regex = Regex::new(r"LIVESTREET_SECURITY_KEY = '(.+)'").unwrap();

        let page = try!(user.get(&"/login".to_owned()));
        let page = try_to_parse!(
            page.find(Name("html")).first()
        ).html();

        user.security_ls_key = ls_key_regex.captures(&page).unwrap().at(1).unwrap().to_owned();

        if let (Some(login), Some(pass)) = (login.into(), pass.into()) {
            try!(user.login(login, pass));
        }

        Ok(user)
    }

    ///Заметка себе: создаёт промежуточный объект запроса, сразу выставляя печеньки,
    ///на случай если надо что-то поменять (как в delete_post)
    fn create_middle_req(&mut self, url: &str) -> hyper::client::RequestBuilder {
        let full_url = format!("{}{}", HOST_URL, url); //TODO: Заменить на concat_idents! когда он стабилизируется
        self.client.get(&full_url)
            .header(Cookie::from_cookie_jar(&self.cookies))
    }

    fn get(&mut self,url: &str) -> Result<Document, TabunError> {
        let mut res = try!(self.create_middle_req(url).send());

        if res.status != hyper::Ok {
            return Err(TabunError::from(res.status));
        }

        let mut buf = String::new();
        try!(res.read_to_string(&mut buf));

        if let Some(x) = res.headers.get::<SetCookie>() {
            x.apply_to_cookie_jar(&mut self.cookies);
        }

        Ok(Document::from(&*buf))
    }

    fn post_urlencoded(&mut self, url: &str, bd: HashMap<&str, &str>) -> Result<hyper::client::Response, TabunError> {
        let url = format!("{}{}", HOST_URL, url); //TODO: Заменить на concat_idents! когда он стабилизируется

        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(bd)
            .finish();

        let req = self.client.post(&url)
            .header(Cookie::from_cookie_jar(&self.cookies))
            .header(ContentType(Mime(TopLevel::Application, SubLevel::WwwFormUrlEncoded, vec![])))
            .body(body.as_str());

        let res = try!(req.send());

        if let Some(x) = res.headers.get::<SetCookie>() {
            x.apply_to_cookie_jar(&mut self.cookies);
        }

        if res.status != hyper::Ok && res.status != hyper::status::StatusCode::MovedPermanently {
            return Err(TabunError::from(res.status));
        }

        Ok(res)
    }

    fn multipart(&mut self,url: &str, bd: HashMap<&str,&str>) -> Result<hyper::client::Response, TabunError> {
        let url = format!("{}{}", HOST_URL, url); //TODO: Заменить на concat_idents! когда он стабилизируется
        let mut request = Request::new(
            hyper::method::Method::Post,
            hyper::Url::from_str(&url).unwrap()
        ).unwrap();  // TODO: обработать нормально?
        request.headers_mut().set(Cookie::from_cookie_jar(&self.cookies));

        let mut req = Multipart::from_request(request).unwrap();

        for (param,val) in bd {
            let _ = req.write_text(param,val);
        }

        let res = try!(req.send());

        if let Some(x) = res.headers.get::<SetCookie>() {
            x.apply_to_cookie_jar(&mut self.cookies);
        }

        if res.status != hyper::Ok && res.status != hyper::status::StatusCode::MovedPermanently {
            return Err(TabunError::from(res.status));
        }

        Ok(res)
    }

    ///Логинится с указанными именем пользователя и паролем
    pub fn login(&mut self, login: &str, pass: &str) -> TabunResult<()> {
        let err_regex = Regex::new("\"sMsgTitle\":\"(.+)\",\"sMsg\":\"(.+?)\"").unwrap();

        let key = self.security_ls_key.to_owned();

        let mut res = try!(self.post_urlencoded(
            "/login/ajax-login",
            map![
                "login" => login,
                "password" => pass,
                "return-path" => HOST_URL,
                "remember" => "on",
                "security_ls_key" => &key
            ]
        ));

        let mut bd = String::new();
        try!(res.read_to_string(&mut bd));
        let bd = bd.as_str();

        if bd.contains("Hacking") {
            Err(TabunError::HackingAttempt)
        } else if err_regex.is_match(bd) {
            let err = err_regex.captures(bd).unwrap();
            Err(TabunError::Error(
                    unescape!(err.at(1).unwrap()),
                    unescape!(err.at(2).unwrap())))
        } else {
            let page = try!(self.get(&"/".to_owned()));
            self.name = try_to_parse!(
                page.find(Class("username")).first()
            ).text();

            Ok(())
        }
    }

    ///Загружает картинку по URL, попутно вычищая табуновские бэкслэши из ответа
    pub fn upload_image_from_url(&mut self, url: &str) -> TabunResult<String> {
        let key = self.security_ls_key.to_owned();
        let url_regex = Regex::new(r"img src=\\&quot;(.+)\\&quot;").unwrap();
        let mut res_s = String::new();
        let mut res = try!(self.multipart("/ajax/upload/image", map!["title" => "", "img_url" => url, "security_ls_key" => &key]));
        try!(res.read_to_string(&mut res_s));
        if let Some(x) = url_regex.captures(&res_s) {
            Ok(x.at(1).unwrap().to_owned())
        } else {
            let err_regex = Regex::new("\"sMsgTitle\":\"(.+)\",\"sMsg\":\"(.+?)\"").unwrap();
            let s = res_s.to_owned();
            let err = try_to_parse!(err_regex.captures(&s));
            Err(TabunError::Error(
                    unescape!(err.at(1).unwrap()),
                    unescape!(err.at(2).unwrap())))
        }
    }

    ///Получает ID блога по его имени
    ///
    ///# Examples
    ///```no_run
    ///# let mut user = libtabun::TClient::new("логин","пароль").unwrap();
    ///let blog_id = user.get_blog_id("lighthouse").unwrap();
    ///assert_eq!(blog_id,15558);
    ///```
    pub fn get_blog_id(&mut self,name: &str) -> TabunResult<u32> {
        let url = format!("/blog/{}", name);
        let page = try!(self.get(&url));

        Ok(try_to_parse!(hado!{
            el <- page.find(And(Name("div"),Class("vote-item"))).find(Name("span")).first();
            id_s <- el.attr("id");
            num_s <- id_s.split('_').last();
            num_s.parse::<u32>().ok()
        }))
    }

    ///Получает инфу о пользователе,
    ///если указан как None, то получает инфу о
    ///текущем пользователе
    ///
    ///# Examples
    ///```no_run
    ///# let mut user = libtabun::TClient::new("логин","пароль").unwrap();
    ///user.get_profile("Orhideous");
    pub fn get_profile<'f, T: Into<Option<&'f str>>>(&mut self, name: T) -> TabunResult<UserInfo> {
        let name = match name.into() {
            Some(x) => x.to_owned(),
            None    => self.name.to_owned()
        };

        let full_url = format!("/profile/{}", name);
        let page = try!(self.get(&full_url));
        let profile = page.find(And(Name("div"),Class("profile")));

        let username = try_to_parse!(
                profile.find(And(Name("h2"),Attr("itemprop","nickname"))).first()
            ).text();

        let realname = match profile.find(And(Name("p"),Attr("itemprop","name"))).first() {
                Some(x) => x.text(),
                None => String::new()
        };

        let (skill,user_id) = try_to_parse!(hado!{
            skill_area <- profile.find(And(Name("div"),Class("strength"))).find(Name("div")).first();
            skill <- skill_area.text().parse::<f32>().ok();
            user_id <- hado!{
                id_s <- skill_area.attr("id");
                elm <- id_s.split('_').collect::<Vec<_>>().get(2);
                elm.parse::<u32>().ok()
            };
            Some((skill,user_id))
        });

        let rating = try_to_parse!(hado!{
            el <- profile.find(Class("vote-count")).find(Name("span")).first();
            el.text().parse::<f32>().ok()
        });

        let about = try_to_parse!(page.find(And(Name("div"),Class("profile-info-about"))).first());

        let userpic = try_to_parse!(about.find(Class("avatar")).find(Name("img")).first());
        let userpic = try_to_parse!(userpic.attr("src"));

        let description = try_to_parse!(about.find(And(Name("div"),Class("text"))).first()).inner_html();

        let dotted = page.find(And(Name("ul"), Class("profile-dotted-list")));
        let dotted = try_to_parse!(dotted.iter().last()).find(Name("li"));

        let mut other_info = HashMap::<String,String>::new();

        let mut created = Vec::<String>::new();
        let mut admin = Vec::<String>::new();
        let mut moderator = Vec::<String>::new();
        let mut member= Vec::<String>::new();

        for li in dotted.iter() {
            let name = try_to_parse!(li.find(Name("span")).first()).text();
            let val = try_to_parse!(li.find(Name("strong")).first());

            if name.contains("Создал"){
                created = val.find(Name("a")).iter().map(|x| x.text()).collect::<Vec<_>>();
            } else if name.contains("Администрирует") {
                admin = val.find(Name("a")).iter().map(|x| x.text()).collect::<Vec<_>>();
            } else if name.contains("Модерирует") {
                moderator = val.find(Name("a")).iter().map(|x| x.text()).collect::<Vec<_>>();
            } else if name.contains("Состоит") {
                member = val.find(Name("a")).iter().map(|x| x.text()).collect::<Vec<_>>();
            } else {
                other_info.insert(name.replace(":",""),val.text());
            }
        }

        let blogs = InBlogs{
            created: created,
            admin: admin,
            moderator: moderator,
            member: member
        };

        let nav = page.find(Class("nav-profile")).find(Name("li"));

        let (mut publications,mut favourites, mut friends) = (0,0,0);

        for li in nav.iter() {
            let a = try_to_parse!(li.find(Name("a")).first()).text();

            if !a.contains("Инфо") {
                 let a = a.split('(').collect::<Vec<_>>();
                 if a.len() >1 {
                     let val = try_to_parse!(a[1].replace(")","")
                         .parse::<u32>().ok());
                     if a[0].contains(&"Публикации") {
                         publications = val
                     } else if a[0].contains(&"Избранное") {
                         favourites = val
                     } else {
                         friends = val
                     }
                 }
            }
        }

        Ok(UserInfo{
            username:       username,
            realname:       realname,
            skill:          skill,
            id:             user_id,
            rating:         rating,
            userpic:        userpic.to_owned(),
            description:    description,
            other_info:     other_info,
            blogs:          blogs,
            publications:   publications,
            favourites:     favourites,
            friends:        friends
        })
    }

    ///Добавляет что-то в избранное, true - коммент, false - пост
    ///(внутренний метод для публичных favourite_post и favourite_comment)
    fn favourite(&mut self, id: u32, typ: bool, fn_typ: bool) -> TabunResult<u32> {
        let id = id.to_string();
        let key = self.security_ls_key.to_owned();

        let body = map![
        if fn_typ { "idComment"} else { "idTopic" } => id.as_str(),
        "type"                                      => &(if typ { "1" } else { "0" }),
        "security_ls_key"                           => &key
        ];

        let mut res = try!(self.multipart(&format!("/ajax/favourite/{}/", if fn_typ { "comment" } else { "topic" }),body));

        if res.status != hyper::Ok { return Err(TabunError::NumError(res.status)) }

        let mut bd = String::new();
        try!(res.read_to_string(&mut bd));

        if bd.contains("\"bStateError\":true") {
            let err = Regex::new("\"sMsgTitle\":\"(.+)\",\"sMsg\":\"(.+?)\"").unwrap().captures(&bd).unwrap();
            Err(TabunError::Error(
                    unescape!(err.at(1).unwrap()),
                    unescape!(err.at(2).unwrap())))
        } else {
            parse_text_to_res!(regex => "\"iCount\":(\\d+)", st => &bd, num => 1, typ => u32)
        }
    }
}

#[cfg(test)]
mod test {
    use ::{TClient};
    use ::regex::{Error,Regex};

    #[test]
    fn test_parsetext_macro() {
        let r : Result<u32, Error> = parse_text_to_res!(regex => r"sometext (\d+) sometext", st => "sometext 001 sometext", num => 1, typ => u32);
        match r {
            Ok(x)   => assert_eq!(x, 1),
            Err(_)  => unreachable!()
        }
    }

    #[test]
    fn test_blog_id() {
        let mut user = TClient::new(None,None).unwrap();
        match user.get_blog_id("herp_derp") {
            Ok(x)   => assert_eq!(193, x),
            Err(x)  => panic!(x)
        }
    }

    #[test]
    fn test_get_profile() {
        let mut user = TClient::new(None,None).unwrap();
        match user.get_profile("OrHiDeOuS") {
            Ok(x)   => assert_eq!(x.username, "Orhideous"),
            Err(x)  => panic!(x)
        }
    }
}
