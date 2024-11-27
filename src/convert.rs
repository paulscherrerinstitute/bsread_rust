use num_traits::{NumCast, ToPrimitive};

pub fn try_convert_num_arr<T, U>(input: &Vec<T>) -> Option<Vec<U>>
where
    T: ToPrimitive + Clone,  // T must implement ToPrimitive
    U: NumCast,       // U must implement NumCast
{
    //TODO: if U==T can I return just a reference to the input?
    // if TypeId::of::<T>() == TypeId::of::<U>() {
    //    return Some(input);
    //}
    input.iter().map(|item| U::from(item.clone())).collect()
}

pub fn try_convert_str_arr<T: ToString>(input: &Vec<T>) -> Option<Vec<String>> {
    // Map each item to its string representation and collect the results into a Vec<String>
    Some(input.iter().map(|item| item.to_string()).collect())
}

pub fn try_convert_bool_arr<U>(input: &Vec<bool>) -> Option<Vec<U>>
where
    U: NumCast,       // U must implement NumCast
{
    //TODO: if U==T can I return just a reference to the input?
    // if TypeId::of::<T>() == TypeId::of::<U>() {
    //    return Some(input);
    //}
    //input.iter().map(|item| U::from(item.clone())).collect()
    let au32: Vec<u32> =  input.iter().map(|b| if *b { 1 } else { 0 }).collect();
    //TODO: if U==u32 can I return just a reference to the input?
    //if TypeId::of::<U>() == TypeId::of::<u32>() {
    //    return Some(au32);
    //}
    try_convert_num_arr::<u32, U>(&au32)
}


pub fn try_convert_num<T, U>(input: &T) -> Option<U>
where
    T: ToPrimitive + Clone,  // T must implement ToPrimitive
    U: NumCast,       // U must implement NumCast
{
    U::from(input.clone())
}


pub fn try_convert_bool<U>(input: &bool) -> Option<U>
where
    U: NumCast,       // U must implement NumCast
{
    let v = if *input { 1 } else { 0 };
    U::from(v)
}
