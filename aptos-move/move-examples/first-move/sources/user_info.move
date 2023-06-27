module sender::user_info {
    //use std::string::String;
    use std::signer;

    struct UserInfo has key {
        age: u8
    }

    public fun get_age(user_addr: address): u8 acquires UserInfo{
        borrow_global<UserInfo>(user_addr).age
    }

    public entry fun set_age(user_account: &signer, age: u8) acquires UserInfo{
        let user_addr = signer::address_of(user_account);
        if (!exists<UserInfo>(user_addr)){
            let info_store = UserInfo{age};
            move_to(user_account, info_store);
        }else{
            let existing_info_store = borrow_global_mut<UserInfo>(user_addr);
            existing_info_store.age = age;
        }
    }
}

#[test_only]
module sender::user_info_tests{
    //use std::string::String;
    use sender::user_info;
    use std::signer;

    #[test(user_account = @042)]
    public entry fun test_getter_setter(user_account: &signer){
        let user_addr = signer::address_of(user_account);
        user_info::set_age(user_account, 10);
        assert!(user_info::get_age(user_addr) == 10, 1);
    }
}
