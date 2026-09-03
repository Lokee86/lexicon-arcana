fn fallback() {}

pub fn swallowed(value: Result<(), ()>) {
    match value {
        Ok(()) => {}
        Err(_) => {}
    }
}

pub fn propagated(value: Result<(), ()>) -> Result<(), ()> {
    match value {
        Ok(()) => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn recorded(value: Result<(), ()>) {
    if let Err(error) = value {
        eprintln!("{error:?}");
    }
}

pub fn recovered(value: Result<(), ()>) {
    match value {
        Ok(()) => {}
        Err(_) => fallback(),
    }
}
