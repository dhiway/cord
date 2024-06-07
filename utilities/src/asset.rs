pub struct Asset {
    status: Status,
}

impl Asset {
    pub fn new() -> Self {
        Asset { status: Status::Pending }
    }

    pub fn activate(&mut self) -> Result<(), &'static str> {
        if self.status == Status::Pending {
            self.status = Status::Active;
            Ok(())
        } else {
            Err("Asset already active")
        }
    }

    pub fn deactivate(&mut self) -> Result<(), &'static str> {
        if self.status == Status::Active {
            self.status = Status::Inactive;
            Ok(())
        } else {
            Err("Asset already inactive")
        }
    }

    pub fn get_status(&self) -> Status {
        self.status
    }
}

pub enum Status {
    Pending,
    Active,
    Inactive,
}
