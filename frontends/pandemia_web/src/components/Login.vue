<template>
  <div class="login">
    <div class="ui center aligned stackable grid">
      <div class="seven wide column center aligned">
        <div class="ui raised very padded container segment">
          <center>
            <img alt="Pandemia logo" src="../assets/logo.png" style="width: 200px;" />

            <h1>{{ title }}</h1>

            <p>{{ desc }}</p>
          </center>
          <div class="ui divider"></div>
          <form class="ui form" method="POST" @submit="doLogin($event)">
            <div class="field">
              <label>Email:</label>
              <input :disabled="isLoading" type="text" name="email" placeholder="Email" ref="inputEmail" />
            </div>
            <div class="field">
              <label>Password:</label>
              <input :disabled="isLoading" type="password" name="password" placeholder="Password" ref="inputPassword" />
            </div>
            <!-- <div class="field">
              <div class="ui checkbox">
                <input type="checkbox" tabindex="0" class="hidden" />
                <label>Remember me</label>
              </div>
            </div>-->
            <center>
              <button :disabled="isLoading" :class=" isLoading ? 'ui loading green large button' : 'ui green large button' " type="submit">
                ENTER</button>
            </center>
          </form>
        </div>
      </div>
    </div>
  </div>
</template>

<script>
export default {
  name: "Login",
  props: {
    title: String,
    desc: String
  },
  data() {
    return {
      token: this.token,
      isLoading: false
    };
  },
  methods: {
    doLogin: function(event) {
      var self = this;
      this.isLoading = true;
      if (event) event.preventDefault();
      this.$pandemia
        .adminLogin(
          this.$refs.inputEmail.value,
          null,
          this.$refs.inputPassword.value
        )
        .then(resp => {
          this.isLoading = false;
          if (resp.data.code == 0) {
            this.$pandemia.getAdminMeInfo().then(self._handleGetMeInfo);
          } else if (resp.data.code == 3000) {
            showLoginError();
          } else if (resp.data.code == 6002) {
            showLoginError();
          } else {
            showLoginError(resp.data.description);
          }
        })
        .catch(_e => {
          this.isLoading = false;
          showLoginError();
        });
      function showLoginError(desc) {
        self.$notify({
          group: "auth",
          title: "Login",
          type: "warn",
          text: desc ? desc : "Wrong email, phone number or password."
        });
      }
    },
    _handleGetMeInfo(_resp) {
      this.$router.push("/dashboard");
    }
  }
};
</script>

<!-- Add "scoped" attribute to limit CSS to this component only -->
<style scoped lang="less">
h3 {
  margin: 40px 0 0;
}
ul {
  list-style-type: none;
  padding: 0;
}
li {
  display: inline-block;
  margin: 0 10px;
}
a {
  color: #42b983;
}
</style>

