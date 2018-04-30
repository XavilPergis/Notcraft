// use std::num::ParseFloatError;
// use cgmath::Vector2;
// use cgmath::Vector3;
// use std::path::Path;
// use std::num::ParseIntError;

// pub trait ObjReciever {
//     fn vertex(&mut self, ctx: &mut Context, vertex: Vector3<f64>);
//     fn tex_coord(&mut self, ctx: &mut Context, tex_coord: Vector2<f64>);
//     fn normal(&mut self, ctx: &mut Context, normal: Vector3<f64>);
//     fn element(&mut self, ctx: &mut Context, element: Element);
// }

// pub struct Context<'s> {
//     current_group: Option<&'s str>,
// }

// pub struct FaceIndex {
//     vertex: usize,
//     tex_coord: Option<usize>,
//     normal: Option<usize>,
// }

// pub enum Element {
//     Point(usize),
//     Line(Vec<usize>),
//     Triangle(FaceIndex, FaceIndex, FaceIndex),
//     Quad(FaceIndex, FaceIndex, FaceIndex, FaceIndex),
//     Polygon(Vec<FaceIndex>),
// }

// // Vertex data:
// //     v    Geometric vertices: v x y z
// //     vt   Texture vertices:   vt u v
// //     vn   Vertex normals:     vn dx dy dz

// // Elements:
// //     p    Point:                    p v1
// //     l    Line:                     l v1 v2 ... vn
// //     f    Face:                     f v1 v2 ... vn
// //     f    Face with texture coords: f v1/t1 v2/t2 .... vn/tn
// //     f    Face with vertex normals: f v1//n1 v2//n2 .... vn//nn
// //     f    Face with txt and norms:  f v1/t1/n1 v2/t2/n2 .... vn/tn/nn

// // Grouping:
// //     g     Group name: g groupname
 
// // Display/render attributes:
// //     usemtl     Material name:    usemtl materialname
// //     mtllib     Material library: mtllib materiallibname.mtl

// enum VertexData {
//     Vertex(Vector3<f64>),
//     TexCoord(Vector2<f64>),
//     Normal(Vector3<f64>),
// }

// enum Line<'s> {
//     VertexData(VertexData),
//     Element(Element),
//     Group(&'s str),
//     Empty,
// }

// pub type ParseResult<T> = Result<T, ParseError>;
// pub enum ParseError {
//     ParseIntError(ParseIntError),
//     ParseFloatError(ParseFloatError),
//     Malformed,
// }

// impl From<ParseIntError> for ParseError {
//     fn from(e: ParseIntError) -> Self { ParseError::ParseIntError(e) }
// }

// impl From<ParseFloatError> for ParseError {
//     fn from(e: ParseFloatError) -> Self { ParseError::ParseFloatError(e) }
// }


// fn parse_point(line: &str) -> ParseResult<Element> {
//     debug_assert!(line.starts_with("p "));
//     match line.split_whitespace().skip(1).next() {
//         Some(val) => Ok(Element::Point(val.parse()?)),
//         None => Err(ParseError::Malformed),
//     }
// }

// fn parse_line(line: &str) -> ParseResult<Element> {
//     debug_assert!(line.starts_with("l "));
//     let elems = line
//         .split_whitespace()
//         .skip(1)
//         .map(|val| val.parse())
//         .collect::<Result<Vec<usize>, _>>()?;
    
//     Ok(Element::Line(elems))
// }

// fn parse_face(line: &str) -> ParseResult<Element> {
//     debug_assert!(line.starts_with("f "));
//     let mut iter = line.split_whitespace().skip(1);
//     let size = line.split_whitespace().skip(1).count();

//     fn parse_vertex(vertex: &str) -> Result<FaceIndex, ParseError> {
//         let mut iter = vertex.split('/');
//         match (iter.next(), iter.next(), iter.next()) {
//             (Some(v), None,     None) => Ok(FaceIndex { vertex: v.parse()?, tex_coord: None, normal: None }),
//             (Some(v), Some(t),  None) => Ok(FaceIndex { vertex: v.parse()?, tex_coord: Some(t.parse()?), normal: None }),
//             (Some(v), Some(""), Some(n)) => Ok(FaceIndex { vertex: v.parse()?, tex_coord: None, normal: Some(n.parse()?) }),
//             (Some(v), Some(t),  Some(n)) => Ok(FaceIndex { vertex: v.parse()?, tex_coord: Some(t.parse()?), normal: Some(n.parse()?) }),
//             _ => Err(ParseError::Malformed)
//         }
//     }

//     Ok(match size {
//         3 => Element::Triangle(
//             parse_vertex(iter.next().unwrap())?,
//             parse_vertex(iter.next().unwrap())?,
//             parse_vertex(iter.next().unwrap())?),
//         4 => Element::Quad(
//             parse_vertex(iter.next().unwrap())?,
//             parse_vertex(iter.next().unwrap())?,
//             parse_vertex(iter.next().unwrap())?,
//             parse_vertex(iter.next().unwrap())?),
//         n => Element::Polygon(iter.map(parse_vertex).collect::<Result<Vec<_>, _>>()?),
//     })
// }

// fn parse_vertex_data(line: &str) -> ParseResult<VertexData> {
//     debug_assert!(line.starts_with("v"));
//     let mut iter = line.split_whitespace().skip(1);
//     match line.chars().skip(1).next() {
//         // oh god kill it with fire
//         None => Ok(VertexData::Vertex(Vector3::new(
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//         ))),
//         Some('t') => Ok(VertexData::TexCoord(Vector2::new(
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//         ))),
//         Some('n') => Ok(VertexData::Normal(Vector3::new(
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//             iter.next().ok_or(ParseError::Malformed)?.parse()?,
//         ))),
//         _ => Err(ParseError::Malformed),
//     }
// }

// fn parse_single_line(line: &str) -> ParseResult<Line> {
//     match line.chars().next() {
//         Some('v') => Ok(Line::VertexData(parse_vertex_data(line)?)),

//         // Elements
//         Some('p') => Ok(Line::Element(parse_point(line)?)),
//         Some('l') => Ok(Line::Element(parse_line(line)?)),
//         Some('f') => Ok(Line::Element(parse_face(line)?)),

//         // TODO: Ignore everything else for now
//         Some(_) | None => Ok(Line::Empty),
//     }
// }

// pub fn parse_file<P: AsRef<Path>, R: ObjReciever>(path: P, recv: &mut R) -> ParseResult<bool> {
//     use std::io::Read;
//     let mut file = ::std::fs::File::open(path).unwrap();
//     let mut buffer = String::new();
//     file.read_to_string(&mut buffer);

//     parse_buffer(&buffer, recv)
// }

// struct Reorganizer<F: Fn(Vector3<f64>, Vector2<f64>, Vector3<f64>)> {
//     vertices: Vec<Vector3<f64>>,
//     uvs: Vec<Vector2<f64>>,
//     normals: Vec<Vector3<f64>>,
//     receiver: F,
// }

// impl<F: Fn(Vector3<f64>, Vector2<f64>, Vector3<f64>)> Reorganizer<F> {
    
// }

// impl<F: Fn(Vector3<f64>, Vector2<f64>, Vector3<f64>)> ObjReciever for Reorganizer<F> {
//     fn vertex(&mut self, ctx: &mut Context, vertex: Vector3<f64>) {
//         self.vertices.push(vertex);
//     }

//     fn tex_coord(&mut self, ctx: &mut Context, tex_coord: Vector2<f64>) {
//         self.uvs.push(tex_coord);
//     }

//     fn normal(&mut self, ctx: &mut Context, normal: Vector3<f64>) {
//         self.normals.push(normal);
//     }

//     fn element(&mut self, ctx: &mut Context, element: Element) {
        
//     }
// }

// pub fn parse_buffer<T: AsRef<str>, R: ObjReciever>(buffer: T, recv: &mut R) -> ParseResult<bool> {
//     let mut context = Context { current_group: None };

//     let mut groups 

//     for line in buffer.as_ref().lines() {
//         match parse_single_line(line)? {
//             Line::VertexData(VertexData::Vertex(v)) => recv.vertex(&mut context, v),
//             Line::VertexData(VertexData::TexCoord(t)) => recv.tex_coord(&mut context, t),
//             Line::VertexData(VertexData::Normal(n)) => recv.normal(&mut context, n),
//             Line::Element(elem) => recv.element(&mut context, elem),
//             Line::Group(group) => context.current_group = Some(group),
//             _ => ()
//         }
//     }

//     // We've successfully parsed the entire thing
//     Ok(true)
// }
