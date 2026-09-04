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

pub fn fallible() -> Result<(), ()> {
    Ok(())
}

pub fn observe_outcomes() -> Result<(), ()> {
    fallible();
    let _value = fallible();
    fallible()?;
    Ok(())
}

pub struct Worker;

impl Worker {
    pub fn fallible_method(&self) -> Result<(), ()> {
        Ok(())
    }
}

pub fn observe_method_outcomes(worker: &Worker) -> Result<(), ()> {
    worker.fallible_method();
    let _value = worker.fallible_method();
    worker.fallible_method()?;
    Ok(())
}
