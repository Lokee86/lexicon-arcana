module example.com/oracle/api

go 1.22

require (
	example.com/oracle/shared v0.0.0
	example.com/external v1.2.3
)

replace example.com/oracle/shared => ../../shared
replace example.com/external => example.com/fork v1.4.0
