��|�           ��|�   SE Linux Module                   nix   1.0                                  lnk_file      read            object_r@           @           @               	   	                @           etc_t             @           systemd_unit_file_t             @           bin_t             @           usr_t	             @           var_run_t             @           lib_t	             @           default_t             @           man_t   	          @           init_t                                                           @   @                 @               @   @          �       @                                @           @   @                 @           @   @          �      @           @           @           @              @   @                 @           @           @           @           @           @           @           @                                                                                         lnk_file               object_r         	      etc_t            systemd_unit_file_t            bin_t            usr_t         	   var_run_t            lib_t         	   default_t            man_t            init_t                             ��|�/nix/store/[^/]+/s?bin(/.*)?	system_u:object_r:bin_t:s0
/nix/store/[^/]+/lib/systemd/system(/.*)?	system_u:object_r:systemd_unit_file_t:s0
/nix/store/[^/]+/lib(/.*)?	system_u:object_r:lib_t:s0
/nix/store/[^/]+/man(/.*)?	system_u:object_r:man_t:s0
/nix/store/[^/]+/etc(/.*)?	system_u:object_r:etc_t:s0
/nix/store/[^/]+/share(/.*)?	system_u:object_r:usr_t:s0
/nix/var/nix/daemon-socket(/.*)?	system_u:object_r:var_run_t:s0
/nix/var/nix/profiles(/per-user/[^/]+)?/[^/]+	system_u:object_r:usr_t:s0

/nix/determinate/determinate-nixd	system_u:object_r:bin_t:s0
/nix/var/determinate/determinate-nixd.socket	system_u:object_r:var_run_t:s0
/nix/var/determinate/intake.pipe	system_u:object_r:var_run_t:s0
/nix/var/determinate/post-build-hook.sh	system_u:object_r:bin_t:s0
/nix/var/determinate/netrc	system_u:object_r:etc_t:s0
